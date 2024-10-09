use chrono::{Datelike, Local, Timelike};
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    execute, style,
    style::Print,
    terminal::{self, ClearType},
};
use futures::{future::FutureExt, StreamExt};
use lib::types::core::{
    DebugCommand, DebugSender, Identity, KernelMessage, Message, MessageSender, PrintReceiver,
    PrintSender, Printout, Request, TERMINAL_PROCESS_ID,
};
use std::{
    fs::{read_to_string, OpenOptions},
    io::BufWriter,
};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use unicode_segmentation::UnicodeSegmentation;

pub mod utils;

struct State {
    pub stdout: std::io::Stdout,
    /// handle and settings for on-disk log (disabled by default, triggered by CTRL+L)
    pub logger: utils::Logger,
    /// in-memory searchable command history that persists itself on disk (default size: 1000)
    pub command_history: utils::CommandHistory,
    /// terminal window width, 0 is leftmost column
    pub win_cols: u16,
    /// terminal window height, 0 is topmost row
    pub win_rows: u16,
    /// the input line (bottom row)
    pub current_line: CurrentLine,
    /// flag representing whether we are in step-through mode (activated by CTRL+J, stepped by CTRL+S)
    pub in_step_through: bool,
    /// flag representing whether we are in search mode (activated by CTRL+R, exited by CTRL+G)
    pub search_mode: bool,
    /// depth of search mode (activated by CTRL+R, increased by CTRL+R)
    pub search_depth: usize,
    /// flag representing whether we are in logging mode (activated by CTRL+L)
    pub logging_mode: bool,
    /// verbosity mode (increased by CTRL+V)
    pub verbose_mode: u8,
}

impl State {
    fn display_current_input_line(&mut self, show_end: bool) -> Result<(), std::io::Error> {
        execute!(
            self.stdout,
            cursor::MoveTo(0, self.win_rows),
            terminal::Clear(ClearType::CurrentLine),
            style::SetForegroundColor(style::Color::Reset),
            Print(self.current_line.prompt),
            Print(utils::truncate_in_place(
                &self.current_line.line,
                self.win_cols - self.current_line.prompt_len as u16,
                self.current_line.line_col,
                self.current_line.cursor_col,
                show_end
            )),
            cursor::MoveTo(
                self.current_line.prompt_len as u16 + self.current_line.cursor_col,
                self.win_rows
            ),
        )
    }

    fn search(&mut self, our_name: &str) -> Result<(), std::io::Error> {
        let search_prompt = format!("{} *", our_name);
        let search_query = &self.current_line.line;
        if let Some(result) = self.command_history.search(search_query, self.search_depth) {
            let (result_underlined, search_cursor_col) = utils::underline(result, search_query);
            execute!(
                self.stdout,
                cursor::MoveTo(0, self.win_rows),
                terminal::Clear(terminal::ClearType::CurrentLine),
                style::SetForegroundColor(style::Color::Reset),
                style::Print(&search_prompt),
                style::Print(utils::truncate_in_place(
                    &result_underlined,
                    self.win_cols - self.current_line.prompt_len as u16,
                    self.current_line.line_col,
                    search_cursor_col,
                    false,
                )),
                cursor::MoveTo(
                    self.current_line.prompt_len as u16 + search_cursor_col,
                    self.win_rows
                ),
            )
        } else {
            execute!(
                self.stdout,
                cursor::MoveTo(0, self.win_rows),
                terminal::Clear(terminal::ClearType::CurrentLine),
                style::SetForegroundColor(style::Color::Reset),
                style::Print(&search_prompt),
                style::Print(utils::truncate_in_place(
                    &format!("{}: no results", &self.current_line.line),
                    self.win_cols - self.current_line.prompt_len as u16,
                    self.current_line.line_col,
                    self.current_line.cursor_col,
                    false,
                )),
                cursor::MoveTo(
                    self.current_line.prompt_len as u16 + self.current_line.cursor_col,
                    self.win_rows
                ),
            )
        }
    }
}

struct CurrentLine {
    /// prompt for user input (e.g. "mynode.os > ")
    pub prompt: &'static str,
    pub prompt_len: usize,
    /// the grapheme index of the cursor in the current line
    pub line_col: usize,
    /// the column index of the cursor in the terminal window (not including prompt)
    pub cursor_col: u16,
    /// the line itself, which does not include the prompt
    pub line: String,
}

impl CurrentLine {
    fn byte_index(&self) -> usize {
        self.line
            .grapheme_indices(true)
            .nth(self.line_col)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.line.len())
    }

    fn current_char_left(&self) -> Option<&str> {
        if self.line_col == 0 {
            None
        } else {
            self.line.graphemes(true).nth(self.line_col - 1)
        }
    }

    fn current_char_right(&self) -> Option<&str> {
        self.line.graphemes(true).nth(self.line_col)
    }

    fn insert_char(&mut self, c: char) {
        let byte_index = self.byte_index();
        self.line.insert(byte_index, c);
    }

    fn insert_str(&mut self, s: &str) {
        let byte_index = self.byte_index();
        self.line.insert_str(byte_index, s);
    }

    /// returns the deleted character
    fn delete_char(&mut self) -> String {
        let byte_index = self.byte_index();
        let next_grapheme = self.line[byte_index..]
            .graphemes(true)
            .next()
            .map(|g| g.len())
            .unwrap_or(0);
        self.line
            .drain(byte_index..byte_index + next_grapheme)
            .collect()
    }
}

/// main entry point for terminal process
/// called by main.rs
pub async fn terminal(
    our: Identity,
    version: &str,
    home_directory_path: String,
    mut event_loop: MessageSender,
    mut debug_event_loop: DebugSender,
    mut print_tx: PrintSender,
    mut print_rx: PrintReceiver,
    is_detached: bool,
    verbose_mode: u8,
    is_logging: bool,
    max_log_size: Option<u64>,
    number_log_files: Option<u64>,
) -> anyhow::Result<()> {
    let (stdout, _maybe_raw_mode) = utils::splash(&our, version, is_detached)?;

    let (win_cols, win_rows) = crossterm::terminal::size().unwrap_or_else(|_| (0, 0));

    let (prompt, prompt_len) = utils::make_prompt(&our.name);
    let cursor_col: u16 = 0;
    let line_col: usize = 0;

    let in_step_through = false;

    let search_mode = false;
    let search_depth: usize = 0;

    let logging_mode = is_logging;

    // the terminal stores the most recent 1000 lines entered by user
    // in history. TODO should make history size adjustable.
    let history_path = std::fs::canonicalize(&home_directory_path)
        .expect("terminal: could not get path for .terminal_history file")
        .join(".terminal_history");
    let history = read_to_string(&history_path).unwrap_or_default();
    let history_handle = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&history_path)
        .expect("terminal: could not open/create .terminal_history");
    let history_writer = BufWriter::new(history_handle);
    let command_history = utils::CommandHistory::new(1000, history, history_writer);

    // if CTRL+L is used to turn on logging, all prints to terminal
    // will also be written with their full timestamp to the .terminal_log file.
    // logging mode is always on by default
    let log_dir_path = std::fs::canonicalize(&home_directory_path)
        .expect("terminal: could not get path for .terminal_logs dir")
        .join(".terminal_logs");
    let logger = utils::Logger::new(log_dir_path, max_log_size, number_log_files);

    let mut state = State {
        stdout,
        logger,
        command_history,
        win_cols,
        win_rows,
        current_line: CurrentLine {
            prompt,
            prompt_len,
            line_col,
            cursor_col,
            line: "".to_string(),
        },
        in_step_through,
        search_mode,
        search_depth,
        logging_mode,
        verbose_mode,
    };


    // use to trigger cleanup if receive signal to kill process
    #[cfg(unix)]
    let (mut sigalrm, mut sighup, mut sigint, mut sigpipe, mut sigquit, mut sigterm, mut sigusr1, mut sigusr2) = (
        signal(SignalKind::alarm()).expect("terminal: failed to set up SIGALRM handler"),
        signal(SignalKind::hangup()).expect("terminal: failed to set up SIGHUP handler"),
        signal(SignalKind::interrupt()).expect("terminal: failed to set up SIGINT handler"),
        signal(SignalKind::pipe()).expect("terminal: failed to set up SIGPIPE handler"),
        signal(SignalKind::quit()).expect("terminal: failed to set up SIGQUIT handler"),
        signal(SignalKind::terminate()).expect("terminal: failed to set up SIGTERM handler"),
        signal(SignalKind::user_defined1()).expect("terminal: failed to set up SIGUSR1 handler"),
        signal(SignalKind::user_defined2()).expect("terminal: failed to set up SIGUSR2 handler"),
    );

    // if the verbosity boot flag was **not** set to "full event loop", tell kernel
    // the kernel will try and print all events by default so that booting with
    // verbosity mode 3 guarantees all events from boot are shown.
    if verbose_mode != 3 {
        debug_event_loop
            .send(DebugCommand::ToggleEventLoop)
            .await
            .expect("failed to toggle full event loop off");
    }

    // only create event stream if not in detached mode
    if !is_detached {
        let mut reader = EventStream::new();
        loop {
            #[cfg(unix)]
            tokio::select! {
                Some(printout) = print_rx.recv() => {
                    handle_printout(printout, &mut state)?;
                }
                Some(Ok(event)) = reader.next().fuse() => {
                    if handle_event(&our, event, &mut state, &mut event_loop, &mut debug_event_loop, &mut print_tx).await? {
                        break;
                    }
                }
                _ = sigalrm.recv() => return Err(anyhow::anyhow!("exiting due to SIGALRM")),
                _ = sighup.recv() =>  return Err(anyhow::anyhow!("exiting due to SIGHUP")),
                _ = sigint.recv() =>  return Err(anyhow::anyhow!("exiting due to SIGINT")),
                _ = sigpipe.recv() => continue, // IGNORE SIGPIPE!
                _ = sigquit.recv() => return Err(anyhow::anyhow!("exiting due to SIGQUIT")),
                _ = sigterm.recv() => return Err(anyhow::anyhow!("exiting due to SIGTERM")),
                _ = sigusr1.recv() => return Err(anyhow::anyhow!("exiting due to SIGUSR1")),
                _ = sigusr2.recv() => return Err(anyhow::anyhow!("exiting due to SIGUSR2")),
            }
            #[cfg(target_os = "windows")]
            tokio::select! {
                Some(printout) = print_rx.recv() => {
                    handle_printout(printout, &mut state)?;
                }
                Some(Ok(event)) = reader.next().fuse() => {
                    if handle_event(&our, event, &mut state, &mut event_loop, &mut debug_event_loop, &mut print_tx).await? {
                        break;
                    }
                }
            }
        }
    } else {
        loop {
            #[cfg(unix)]
            tokio::select! {
                Some(printout) = print_rx.recv() => {
                    handle_printout(printout, &mut state)?;
                }
                _ = sigalrm.recv() => return Err(anyhow::anyhow!("exiting due to SIGALRM")),
                _ = sighup.recv() =>  return Err(anyhow::anyhow!("exiting due to SIGHUP")),
                _ = sigint.recv() =>  return Err(anyhow::anyhow!("exiting due to SIGINT")),
                _ = sigpipe.recv() => continue, // IGNORE SIGPIPE!
                _ = sigquit.recv() => return Err(anyhow::anyhow!("exiting due to SIGQUIT")),
                _ = sigterm.recv() => return Err(anyhow::anyhow!("exiting due to SIGTERM")),
                _ = sigusr1.recv() => return Err(anyhow::anyhow!("exiting due to SIGUSR1")),
                _ = sigusr2.recv() => return Err(anyhow::anyhow!("exiting due to SIGUSR2")),
            }
            #[cfg(target_os = "windows")]
            if let Some(printout) = print_rx.recv().await {
                handle_printout(printout, &mut state)?;
            };
        }
    };
    Ok(())
}

fn handle_printout(printout: Printout, state: &mut State) -> anyhow::Result<()> {
    // lock here so that runtime can still use println! without freezing..
    // can lock before loop later if we want to reduce overhead
    let mut stdout = state.stdout.lock();
    // always write print to log if in logging mode
    if state.logging_mode {
        state.logger.write(&printout.content)?;
    }
    // skip writing print to terminal if it's of a greater
    // verbosity level than our current mode
    if printout.verbosity > state.verbose_mode {
        return Ok(());
    }
    let now = Local::now();
    execute!(
        stdout,
        // print goes immediately above the dedicated input line at bottom
        cursor::MoveTo(0, state.win_rows - 1),
        terminal::Clear(ClearType::CurrentLine),
        Print(format!(
            "{} {:02}:{:02} ",
            now.weekday(),
            now.hour(),
            now.minute(),
        )),
        style::SetForegroundColor(match printout.verbosity {
            0 => style::Color::Reset,
            1 => style::Color::Green,
            2 => style::Color::Magenta,
            _ => style::Color::Red,
        }),
    )?;
    for line in printout.content.lines() {
        execute!(stdout, Print(format!("{line}\r\n")))?;
    }
    // re-display the current input line
    state.display_current_input_line(false)?;
    Ok(())
}

/// returns True if runtime should exit due to CTRL+C or CTRL+D
async fn handle_event(
    our: &Identity,
    event: Event,
    state: &mut State,
    event_loop: &mut MessageSender,
    debug_event_loop: &mut DebugSender,
    print_tx: &mut PrintSender,
) -> anyhow::Result<bool> {
    let State {
        stdout,
        command_history,
        win_cols,
        win_rows,
        current_line,
        in_step_through,
        search_depth,
        logging_mode,
        verbose_mode,
        ..
    } = state;
    // lock here so that runtime can still use println! without freezing..
    // can lock before loop later if we want to reduce overhead
    let mut stdout = stdout.lock();
    match event {
        //
        // RESIZE: resize is super annoying because this event trigger often
        // comes "too late" to stop terminal from messing with the
        // already-printed lines. TODO figure out the right way
        // to compensate for this cross-platform and do this in a
        // generally stable way.
        //
        Event::Resize(width, height) => {
            // this is critical at moment of resize not to double-up lines
            execute!(
                state.stdout,
                cursor::MoveTo(0, height),
                terminal::Clear(ClearType::CurrentLine)
            )?;
            *win_cols = width - 1;
            *win_rows = height;
            if current_line.cursor_col + current_line.prompt_len as u16 > *win_cols {
                current_line.cursor_col = *win_cols - current_line.prompt_len as u16;
                // can't do this because of wide graphemes :/
                // current_line.line_col = current_line.cursor_col as usize;
            }
        }
        //
        // PASTE: handle pasting of text from outside
        //
        Event::Paste(pasted) => {
            // strip out control characters and newlines
            let pasted = pasted
                .chars()
                .filter(|c| !c.is_control() && !c.is_ascii_control())
                .collect::<String>();
            current_line.insert_str(&pasted);
            current_line.line_col = current_line.line_col + pasted.graphemes(true).count();
            current_line.cursor_col = std::cmp::min(
                current_line.cursor_col + utils::display_width(&pasted) as u16,
                *win_cols - current_line.prompt_len as u16,
            );
        }
        //
        // CTRL+C, CTRL+D: turn off the node
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            execute!(
                stdout,
                // print goes immediately above the dedicated input line at bottom
                cursor::MoveTo(0, *win_rows - 1),
                terminal::Clear(ClearType::CurrentLine),
                Print("exit code received"),
            )?;
            return Ok(true);
        }
        //
        // CTRL+V: toggle through verbosity modes
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('v'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            // go from low to high, then reset to 0
            match verbose_mode {
                0 => *verbose_mode = 1,
                1 => *verbose_mode = 2,
                2 => {
                    *verbose_mode = 3;
                    debug_event_loop
                        .send(DebugCommand::ToggleEventLoop)
                        .await
                        .expect("failed to toggle ON full event loop");
                }
                3 => {
                    *verbose_mode = 0;
                    debug_event_loop
                        .send(DebugCommand::ToggleEventLoop)
                        .await
                        .expect("failed to toggle OFF full event loop");
                }
                _ => unreachable!(),
            }
            Printout::new(
                0,
                format!(
                    "verbose mode: {}",
                    match verbose_mode {
                        0 => "off",
                        1 => "debug",
                        2 => "super-debug",
                        3 => "full event loop",
                        _ => unreachable!(),
                    }
                ),
            )
            .send(&print_tx)
            .await;
            return Ok(false);
        }
        //
        // CTRL+J: toggle debug mode -- makes system-level event loop step-through
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            let _ = debug_event_loop.send(DebugCommand::ToggleStepthrough).await;
            *in_step_through = !*in_step_through;
            Printout::new(
                0,
                format!(
                    "debug mode {}",
                    match in_step_through {
                        false => "off",
                        true => "on: use CTRL+S to step through events",
                    }
                ),
            )
            .send(&print_tx)
            .await;
            return Ok(false);
        }
        //
        // CTRL+S: step through system-level event loop (when in step-through mode)
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            let _ = debug_event_loop.send(DebugCommand::Step).await;
            return Ok(false);
        }
        //
        //  CTRL+L: toggle logging mode
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            *logging_mode = !*logging_mode;
            Printout::new(
                0,
                format!("logging mode: {}", if *logging_mode { "on" } else { "off" }),
            )
            .send(&print_tx)
            .await;
            return Ok(false);
        }
        //
        //  UP / CTRL+P: go up one command in history
        //
        Event::Key(KeyEvent {
            code: KeyCode::Up, ..
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            if state.search_mode {
                return Ok(false);
            }
            // go up one command in history
            match command_history.get_prev(&current_line.line) {
                Some(line) => {
                    let width = utils::display_width(&line);
                    current_line.line_col = line.graphemes(true).count();
                    current_line.line = line;
                    current_line.cursor_col =
                        std::cmp::min(width as u16, *win_cols - current_line.prompt_len as u16);
                }
                None => {
                    // the "no-no" ding
                    print!("\x07");
                }
            }
            state.display_current_input_line(true)?;
            return Ok(false);
        }
        //
        //  DOWN / CTRL+N: go down one command in history
        //
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            ..
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            if state.search_mode {
                return Ok(false);
            }
            // go down one command in history
            match command_history.get_next() {
                Some(line) => {
                    let width = utils::display_width(&line);
                    current_line.line_col = line.graphemes(true).count();
                    current_line.line = line;
                    current_line.cursor_col =
                        std::cmp::min(width as u16, *win_cols - current_line.prompt_len as u16);
                }
                None => {
                    // the "no-no" ding
                    print!("\x07");
                }
            }
            state.display_current_input_line(true)?;
            return Ok(false);
        }
        //
        //  CTRL+A: jump to beginning of line
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            if state.search_mode {
                return Ok(false);
            }
            current_line.line_col = 0;
            current_line.cursor_col = 0;
        }
        //
        //  CTRL+E: jump to end of line
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            if state.search_mode {
                return Ok(false);
            }
            current_line.line_col = current_line.line.graphemes(true).count();
            current_line.cursor_col = std::cmp::min(
                utils::display_width(&current_line.line) as u16,
                *win_cols - current_line.prompt_len as u16,
            );
        }
        //
        //  CTRL+R: enter search mode
        //  if already in search mode, increase search depth
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            if state.search_mode {
                *search_depth += 1;
            }
            state.search_mode = true;
        }
        //
        //  CTRL+G: exit search mode
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            // just show true current line as usual
            state.search_mode = false;
            *search_depth = 0;
        }
        //
        //  KEY: handle keypress events
        //
        Event::Key(k) => {
            match k.code {
                //
                //  CHAR: write a single character
                //
                KeyCode::Char(c) => {
                    current_line.insert_char(c);
                    if (current_line.cursor_col + current_line.prompt_len as u16) < *win_cols {
                        current_line.cursor_col += utils::display_width(&c.to_string()) as u16;
                    }
                    current_line.line_col += 1;
                }
                //
                //  BACKSPACE: delete a single character at cursor
                //
                KeyCode::Backspace => {
                    if current_line.line_col == 0 {
                        return Ok(false);
                    } else {
                        current_line.line_col -= 1;
                        let c = current_line.delete_char();
                        current_line.cursor_col -= utils::display_width(&c) as u16;
                    }
                }
                //
                //  DELETE: delete a single character at right of cursor
                //
                KeyCode::Delete => {
                    if current_line.line_col == current_line.line.graphemes(true).count() {
                        return Ok(false);
                    }
                    current_line.delete_char();
                }
                //
                //  LEFT: move cursor one spot left
                //
                KeyCode::Left => {
                    if current_line.cursor_col as usize == 0 {
                        if current_line.line_col == 0 {
                            // at the very beginning of the current typed line
                            return Ok(false);
                        } else {
                            // virtual scroll leftward through line
                            current_line.line_col -= 1;
                        }
                    } else {
                        // simply move cursor and line position left
                        let width = current_line
                            .current_char_left()
                            .map_or_else(|| 1, |c| utils::display_width(&c))
                            as u16;
                        execute!(stdout, cursor::MoveLeft(width))?;
                        current_line.cursor_col -= width;
                        if current_line.line_col != 0 {
                            current_line.line_col -= 1;
                        }
                        return Ok(false);
                    }
                }
                //
                //  RIGHT: move cursor one spot right
                //
                KeyCode::Right => {
                    if current_line.line_col == current_line.line.graphemes(true).count() {
                        // at the very end of the current typed line
                        return Ok(false);
                    };
                    if (current_line.cursor_col + current_line.prompt_len as u16) < (*win_cols - 1)
                    {
                        // simply move cursor and line position right
                        let width = current_line
                            .current_char_right()
                            .map_or_else(|| 1, |c| utils::display_width(&c))
                            as u16;
                        execute!(stdout, cursor::MoveRight(width))?;
                        current_line.cursor_col += width;
                        current_line.line_col += 1;
                        return Ok(false);
                    } else {
                        // virtual scroll rightward through line
                        current_line.line_col += 1;
                    }
                }
                //
                //  ENTER: send current input to terminal process, clearing input line
                //
                KeyCode::Enter => {
                    // if we were in search mode, pull command from that
                    let command = if !state.search_mode {
                        current_line.line.clone()
                    } else {
                        command_history
                            .search(&current_line.line, *search_depth)
                            .unwrap_or_default()
                            .to_string()
                    };
                    execute!(
                        stdout,
                        cursor::MoveTo(0, *win_rows),
                        terminal::Clear(ClearType::CurrentLine),
                        Print(&current_line.prompt),
                        Print(&command),
                        Print("\r\n"),
                    )?;
                    state.search_mode = false;
                    *search_depth = 0;
                    current_line.cursor_col = 0;
                    current_line.line_col = 0;
                    command_history.add(command.to_string());
                    KernelMessage::builder()
                        .id(rand::random())
                        .source((our.name.as_str(), TERMINAL_PROCESS_ID.clone()))
                        .target((our.name.as_str(), TERMINAL_PROCESS_ID.clone()))
                        .message(Message::Request(Request {
                            inherit: false,
                            expects_response: None,
                            body: command.into_bytes(),
                            metadata: None,
                            capabilities: vec![],
                        }))
                        .build()
                        .unwrap()
                        .send(&event_loop)
                        .await;
                    current_line.line = "".to_string();
                }
                _ => {
                    // some keycode we don't care about, yet
                }
            }
        }
        _ => {
            // some terminal event we don't care about, yet
        }
    }
    if state.search_mode {
        state.search(&our.name)?;
    } else {
        state.display_current_input_line(false)?;
    }
    Ok(false)
}
