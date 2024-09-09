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
    io::{BufWriter, Write},
};
use tokio::signal::unix::{signal, SignalKind};

pub mod utils;

pub struct State {
    pub stdout: std::io::Stdout,
    pub log_writer: BufWriter<std::fs::File>,
    pub command_history: utils::CommandHistory,
    pub win_cols: u16,
    pub win_rows: u16,
    pub prompt_len: usize,
    pub line_col: usize,
    pub cursor_col: u16,
    pub current_line: String,
    pub in_step_through: bool,
    pub search_mode: bool,
    pub search_depth: usize,
    pub logging_mode: bool,
    pub verbose_mode: u8,
}

/*
 *  terminal driver
 */
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
) -> anyhow::Result<()> {
    let (stdout, _maybe_raw_mode) = utils::startup(&our, version, is_detached)?;

    let (win_cols, win_rows) = crossterm::terminal::size().unwrap_or_else(|_| (0, 0));

    let current_line = format!("{} > ", our.name);
    let prompt_len: usize = our.name.len() + 3;
    let cursor_col: u16 = prompt_len as u16;
    let line_col: usize = cursor_col as usize;

    let in_step_through: bool = false;

    let search_mode: bool = false;
    let search_depth: usize = 0;

    let logging_mode: bool = false;

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
    // logging mode is always off by default. TODO add a boot flag to change this.
    let log_path = std::fs::canonicalize(&home_directory_path)
        .expect("terminal: could not get path for .terminal_log file")
        .join(".terminal_log");
    let log_handle = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&log_path)
        .expect("terminal: could not open/create .terminal_log");
    let log_writer = BufWriter::new(log_handle);

    let mut state = State {
        stdout,
        log_writer,
        command_history,
        win_cols,
        win_rows,
        prompt_len,
        line_col,
        cursor_col,
        current_line,
        in_step_through,
        search_mode,
        search_depth,
        logging_mode,
        verbose_mode,
    };

    // use to trigger cleanup if receive signal to kill process
    let mut sigalrm =
        signal(SignalKind::alarm()).expect("terminal: failed to set up SIGALRM handler");
    let mut sighup =
        signal(SignalKind::hangup()).expect("terminal: failed to set up SIGHUP handler");
    let mut sigint =
        signal(SignalKind::interrupt()).expect("terminal: failed to set up SIGINT handler");
    let mut sigpipe =
        signal(SignalKind::pipe()).expect("terminal: failed to set up SIGPIPE handler");
    let mut sigquit =
        signal(SignalKind::quit()).expect("terminal: failed to set up SIGQUIT handler");
    let mut sigterm =
        signal(SignalKind::terminate()).expect("terminal: failed to set up SIGTERM handler");
    let mut sigusr1 =
        signal(SignalKind::user_defined1()).expect("terminal: failed to set up SIGUSR1 handler");
    let mut sigusr2 =
        signal(SignalKind::user_defined2()).expect("terminal: failed to set up SIGUSR2 handler");

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
        }
    } else {
        loop {
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
        }
    };
    Ok(())
}

fn handle_printout(printout: Printout, state: &mut State) -> anyhow::Result<()> {
    // lock here so that runtime can still use println! without freezing..
    // can lock before loop later if we want to reduce overhead
    let mut stdout = state.stdout.lock();
    let now = Local::now();
    // always write print to log if in logging mode
    if state.logging_mode {
        writeln!(
            state.log_writer,
            "[{}] {}",
            now.to_rfc2822(),
            printout.content
        )?;
    }
    // skip writing print to terminal if it's of a greater
    // verbosity level than our current mode
    if printout.verbosity > state.verbose_mode {
        return Ok(());
    }
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
        execute!(stdout, Print(format!("{}\r\n", line)),)?;
    }
    // reset color and re-display the current input line
    // re-place cursor where user had it at input line
    execute!(
        stdout,
        style::ResetColor,
        cursor::MoveTo(0, state.win_rows),
        Print(utils::truncate_in_place(
            &state.current_line,
            state.prompt_len,
            state.win_cols,
            (state.line_col, state.cursor_col)
        )),
        cursor::MoveTo(state.cursor_col, state.win_rows),
    )?;
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
        prompt_len,
        line_col,
        cursor_col,
        current_line,
        in_step_through,
        search_mode,
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
            *win_cols = width;
            *win_rows = height;
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
            current_line.insert_str(*line_col, &pasted);
            *line_col = *line_col + pasted.len();
            *cursor_col = std::cmp::min(
                line_col.to_owned().try_into().unwrap_or(*win_cols),
                *win_cols,
            );
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                Print(utils::truncate_in_place(
                    &current_line,
                    *prompt_len,
                    *win_cols,
                    (*line_col, *cursor_col)
                )),
                cursor::MoveTo(*cursor_col, *win_rows),
            )?;
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
            // go up one command in history
            match command_history.get_prev(&current_line[*prompt_len..]) {
                Some(line) => {
                    *current_line = format!("{} > {}", our.name, line);
                    *line_col = current_line.len();
                }
                None => {
                    // the "no-no" ding
                    print!("\x07");
                }
            }
            *cursor_col = std::cmp::min(current_line.len() as u16, *win_cols);
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                terminal::Clear(ClearType::CurrentLine),
                Print(utils::truncate_rightward(
                    current_line,
                    *prompt_len,
                    *win_cols
                )),
            )?;
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
            // go down one command in history
            match command_history.get_next() {
                Some(line) => {
                    *current_line = format!("{} > {}", our.name, line);
                    *line_col = current_line.len();
                }
                None => {
                    // the "no-no" ding
                    print!("\x07");
                }
            }
            *cursor_col = std::cmp::min(current_line.len() as u16, *win_cols);
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                terminal::Clear(ClearType::CurrentLine),
                Print(utils::truncate_rightward(
                    &current_line,
                    *prompt_len,
                    *win_cols
                )),
            )?;
        }
        //
        //  CTRL+A: jump to beginning of line
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            *line_col = *prompt_len;
            *cursor_col = *prompt_len as u16;
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                Print(utils::truncate_from_left(
                    &current_line,
                    *prompt_len,
                    *win_cols,
                    *line_col
                )),
                cursor::MoveTo(*cursor_col, *win_rows),
            )?;
        }
        //
        //  CTRL+E: jump to end of line
        //
        Event::Key(KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            *line_col = current_line.len();
            *cursor_col = std::cmp::min(
                line_col.to_owned().try_into().unwrap_or(*win_cols),
                *win_cols,
            );
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                Print(utils::truncate_from_right(
                    &current_line,
                    *prompt_len,
                    *win_cols,
                    *line_col
                )),
            )?;
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
            if *search_mode {
                *search_depth += 1;
            }
            *search_mode = true;
            utils::execute_search(
                &our,
                &mut stdout,
                &current_line,
                *prompt_len,
                (*win_cols, *win_rows),
                (*line_col, *cursor_col),
                command_history,
                *search_depth,
            )?;
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
            *search_mode = false;
            *search_depth = 0;
            execute!(
                stdout,
                cursor::MoveTo(0, *win_rows),
                terminal::Clear(ClearType::CurrentLine),
                Print(utils::truncate_in_place(
                    &format!("{} > {}", our.name, &current_line[*prompt_len..]),
                    *prompt_len,
                    *win_cols,
                    (*line_col, *cursor_col)
                )),
                cursor::MoveTo(*cursor_col, *win_rows),
            )?;
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
                    current_line.insert(*line_col, c);
                    if cursor_col < win_cols {
                        *cursor_col += 1;
                    }
                    *line_col += 1;
                    if *search_mode {
                        utils::execute_search(
                            &our,
                            &mut stdout,
                            &current_line,
                            *prompt_len,
                            (*win_cols, *win_rows),
                            (*line_col, *cursor_col),
                            command_history,
                            *search_depth,
                        )?;
                        return Ok(false);
                    }
                    execute!(
                        stdout,
                        cursor::MoveTo(0, *win_rows),
                        terminal::Clear(ClearType::CurrentLine),
                        Print(utils::truncate_in_place(
                            &current_line,
                            *prompt_len,
                            *win_cols,
                            (*line_col, *cursor_col)
                        )),
                        cursor::MoveTo(*cursor_col, *win_rows),
                    )?;
                }
                //
                //  BACKSPACE: delete a single character at cursor
                //
                KeyCode::Backspace => {
                    if line_col == prompt_len {
                        return Ok(false);
                    }
                    if *cursor_col as usize == *line_col {
                        *cursor_col -= 1;
                    }
                    *line_col -= 1;
                    current_line.remove(*line_col);
                    if *search_mode {
                        utils::execute_search(
                            &our,
                            &mut stdout,
                            &current_line,
                            *prompt_len,
                            (*win_cols, *win_rows),
                            (*line_col, *cursor_col),
                            command_history,
                            *search_depth,
                        )?;
                        return Ok(false);
                    }
                    execute!(
                        stdout,
                        cursor::MoveTo(0, *win_rows),
                        terminal::Clear(ClearType::CurrentLine),
                        Print(utils::truncate_in_place(
                            &current_line,
                            *prompt_len,
                            *win_cols,
                            (*line_col, *cursor_col)
                        )),
                        cursor::MoveTo(*cursor_col, *win_rows),
                    )?;
                }
                //
                //  DELETE: delete a single character at right of cursor
                //
                KeyCode::Delete => {
                    if *line_col == current_line.len() {
                        return Ok(false);
                    }
                    current_line.remove(*line_col);
                    if *search_mode {
                        utils::execute_search(
                            &our,
                            &mut stdout,
                            &current_line,
                            *prompt_len,
                            (*win_cols, *win_rows),
                            (*line_col, *cursor_col),
                            command_history,
                            *search_depth,
                        )?;
                        return Ok(false);
                    }
                    execute!(
                        stdout,
                        cursor::MoveTo(0, *win_rows),
                        terminal::Clear(ClearType::CurrentLine),
                        Print(utils::truncate_in_place(
                            &current_line,
                            *prompt_len,
                            *win_cols,
                            (*line_col, *cursor_col)
                        )),
                        cursor::MoveTo(*cursor_col, *win_rows),
                    )?;
                }
                //
                //  LEFT: move cursor one spot left
                //
                KeyCode::Left => {
                    if *cursor_col as usize == *prompt_len {
                        if line_col == prompt_len {
                            // at the very beginning of the current typed line
                            return Ok(false);
                        } else {
                            // virtual scroll leftward through line
                            *line_col -= 1;
                            execute!(
                                stdout,
                                cursor::MoveTo(0, *win_rows),
                                Print(utils::truncate_from_left(
                                    &current_line,
                                    *prompt_len,
                                    *win_cols,
                                    *line_col
                                )),
                                cursor::MoveTo(*cursor_col, *win_rows),
                            )?;
                        }
                    } else {
                        // simply move cursor and line position left
                        execute!(stdout, cursor::MoveLeft(1),)?;
                        *cursor_col -= 1;
                        *line_col -= 1;
                    }
                }
                //
                //  RIGHT: move cursor one spot right
                //
                KeyCode::Right => {
                    if *line_col == current_line.len() {
                        // at the very end of the current typed line
                        return Ok(false);
                    };
                    if *cursor_col < (*win_cols - 1) {
                        // simply move cursor and line position right
                        execute!(stdout, cursor::MoveRight(1))?;
                        *cursor_col += 1;
                        *line_col += 1;
                    } else {
                        // virtual scroll rightward through line
                        *line_col += 1;
                        execute!(
                            stdout,
                            cursor::MoveTo(0, *win_rows),
                            Print(utils::truncate_from_right(
                                &current_line,
                                *prompt_len,
                                *win_cols,
                                *line_col
                            )),
                        )?;
                    }
                }
                //
                //  ENTER: send current input to terminal process, clearing input line
                //
                KeyCode::Enter => {
                    // if we were in search mode, pull command from that
                    let command = if !*search_mode {
                        current_line[*prompt_len..].to_string()
                    } else {
                        command_history
                            .search(&current_line[*prompt_len..], *search_depth)
                            .unwrap_or_default()
                            .to_string()
                    };
                    let next = format!("{} > ", our.name);
                    execute!(
                        stdout,
                        cursor::MoveTo(0, *win_rows),
                        terminal::Clear(ClearType::CurrentLine),
                        Print(&format!("{} > {}", our.name, command)),
                        Print("\r\n"),
                        Print(&next),
                    )?;
                    *search_mode = false;
                    *search_depth = 0;
                    *current_line = next;
                    command_history.add(command.clone());
                    *cursor_col = *prompt_len as u16;
                    *line_col = *prompt_len;
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
    Ok(false)
}
