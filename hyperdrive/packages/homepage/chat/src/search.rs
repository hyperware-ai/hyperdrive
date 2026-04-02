use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crdt::{Group, GroupId, MembershipStatus};
use crate::types::{Chat, ChatMessage};
use crate::{SearchResultItem, SearchResultKind, SearchScope};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;
const MAX_BODY_LEN: usize = 512;
const MAX_CHAT_MESSAGES_PER_CHAT: usize = 5000;
const MAX_GROUP_MESSAGES_PER_GROUP: usize = 5000;
const MESSAGE_WINDOW_SECS: u64 = 180 * 24 * 60 * 60;

#[derive(Clone, Debug)]
struct SearchDoc {
    id: String,
    kind: SearchResultKind,
    chat_id: Option<String>,
    group_id: Option<GroupId>,
    message_id: Option<String>,
    thread_id: Option<String>,
    title: String,
    body: String,
    timestamp: Option<u64>,
    tokens: HashSet<String>,
}

impl SearchDoc {
    fn new(
        id: String,
        kind: SearchResultKind,
        chat_id: Option<String>,
        group_id: Option<GroupId>,
        message_id: Option<String>,
        thread_id: Option<String>,
        title: String,
        body: String,
        timestamp: Option<u64>,
    ) -> Option<Self> {
        let trimmed_title = title.trim();
        let trimmed_body = body.trim();
        let combined = if trimmed_body.is_empty() {
            trimmed_title.to_string()
        } else {
            format!("{trimmed_title} {trimmed_body}")
        };
        let tokens = tokenize(&combined);
        if tokens.is_empty() {
            return None;
        }

        Some(Self {
            id,
            kind,
            chat_id,
            group_id,
            message_id,
            thread_id,
            title: trimmed_title.to_string(),
            body: trimmed_body.to_string(),
            timestamp,
            tokens,
        })
    }
}

#[derive(Default)]
pub(crate) struct SearchIndex {
    docs: HashMap<String, SearchDoc>,
    tokens: HashMap<String, HashSet<String>>,
    chat_doc_ids: HashMap<String, HashSet<String>>,
    group_doc_ids: HashMap<String, HashSet<String>>,
}

impl SearchIndex {
    pub fn clear(&mut self) {
        self.docs.clear();
        self.tokens.clear();
        self.chat_doc_ids.clear();
        self.group_doc_ids.clear();
    }

    pub fn rebuild(
        &mut self,
        chats: &HashMap<String, Chat>,
        groups: &HashMap<GroupId, Group>,
        our_node: &str,
    ) {
        self.clear();
        for chat in chats.values() {
            self.rebuild_chat(&chat.id, chat);
        }
        for (group_id, group) in groups {
            self.rebuild_group(group_id, group, our_node);
        }
    }

    pub fn rebuild_chat(&mut self, chat_id: &str, chat: &Chat) {
        self.remove_chat(chat_id);
        let mut doc_ids = HashSet::new();

        if let Some(summary) = build_chat_summary_doc(chat) {
            doc_ids.insert(summary.id.clone());
            self.insert_doc(summary);
        }

        let cutoff = now_secs().saturating_sub(MESSAGE_WINDOW_SECS);
        let mut messages: Vec<&ChatMessage> = chat
            .messages
            .iter()
            .filter(|m| m.timestamp >= cutoff)
            .collect();
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        for msg in messages.into_iter().take(MAX_CHAT_MESSAGES_PER_CHAT) {
            if msg.content.trim().is_empty() {
                continue;
            }
            if let Some(doc) = build_chat_message_doc(chat, msg) {
                doc_ids.insert(doc.id.clone());
                self.insert_doc(doc);
            }
        }

        if !doc_ids.is_empty() {
            self.chat_doc_ids.insert(chat_id.to_string(), doc_ids);
        }
    }

    pub fn rebuild_group(&mut self, group_id: &GroupId, group: &Group, our_node: &str) {
        self.remove_group(group_id);
        let is_active = group
            .members
            .get(our_node)
            .map(|m| m.status == MembershipStatus::Active)
            .unwrap_or(false);
        if !is_active {
            return;
        }

        let mut doc_ids = HashSet::new();
        if let Some(summary) = build_group_summary_doc(group_id, group) {
            doc_ids.insert(summary.id.clone());
            self.insert_doc(summary);
        }

        let cutoff = now_secs().saturating_sub(MESSAGE_WINDOW_SECS);
        let mut messages: Vec<_> = group
            .messages
            .values()
            .filter(|m| m.timestamp >= cutoff && !m.body.trim().is_empty())
            .collect();
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        for msg in messages.into_iter().take(MAX_GROUP_MESSAGES_PER_GROUP) {
            if let Some(doc) = build_group_message_doc(group_id, group, msg) {
                doc_ids.insert(doc.id.clone());
                self.insert_doc(doc);
            }
        }

        if !doc_ids.is_empty() {
            self.group_doc_ids.insert(group_id.to_string(), doc_ids);
        }
    }

    pub fn remove_chat(&mut self, chat_id: &str) {
        if let Some(ids) = self.chat_doc_ids.remove(chat_id) {
            for id in ids {
                self.remove_doc(&id);
            }
        }
    }

    pub fn remove_group(&mut self, group_id: &GroupId) {
        if let Some(ids) = self.group_doc_ids.remove(group_id) {
            for id in ids {
                self.remove_doc(&id);
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        scope: SearchScope,
        limit: Option<usize>,
    ) -> Vec<SearchResultItem> {
        let clauses: Vec<&str> = query
            .split('|')
            .map(str::trim)
            .filter(|clause| !clause.is_empty())
            .collect();
        if clauses.is_empty() {
            return Vec::new();
        }

        let mut all_tokens: HashSet<String> = HashSet::new();
        let mut score_by_doc: HashMap<String, i64> = HashMap::new();

        for clause in clauses {
            let clause_tokens = tokenize_query(clause);
            if clause_tokens.is_empty() {
                continue;
            }
            for token in &clause_tokens {
                all_tokens.insert(token.clone());
            }

            let mut candidates: Option<HashSet<String>> = None;
            for token in &clause_tokens {
                let docs_for_token = self.docs_for_token(token);
                if docs_for_token.is_empty() {
                    candidates = Some(HashSet::new());
                    break;
                }
                candidates = Some(match candidates {
                    None => docs_for_token,
                    Some(prev) => prev.intersection(&docs_for_token).cloned().collect(),
                });
            }

            let Some(candidate_ids) = candidates else {
                continue;
            };
            if candidate_ids.is_empty() {
                continue;
            }

            for doc_id in candidate_ids {
                let Some(doc) = self.docs.get(&doc_id) else {
                    continue;
                };
                if !doc_matches_scope(doc.kind, scope) {
                    continue;
                }
                let score = score_doc(doc, &clause_tokens);
                if score == 0 {
                    continue;
                }
                let entry = score_by_doc.entry(doc_id).or_insert(score);
                if score > *entry {
                    *entry = score;
                }
            }
        }

        let query_tokens: Vec<String> = all_tokens.into_iter().collect();
        let mut scored: Vec<(i64, u64, &SearchDoc)> = Vec::new();
        for (doc_id, score) in score_by_doc {
            if let Some(doc) = self.docs.get(&doc_id) {
                let ts = doc.timestamp.unwrap_or(0);
                scored.push((score, ts, doc));
            }
        }

        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.title.cmp(&b.2.title))
        });

        let capped = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        scored
            .into_iter()
            .take(capped)
            .map(|(_, _, doc)| doc_to_result(doc, &query_tokens))
            .collect()
    }

    fn insert_doc(&mut self, doc: SearchDoc) {
        let doc_id = doc.id.clone();
        for token in &doc.tokens {
            self.tokens
                .entry(token.clone())
                .or_default()
                .insert(doc_id.clone());
        }
        self.docs.insert(doc_id, doc);
    }

    fn remove_doc(&mut self, doc_id: &str) {
        if let Some(doc) = self.docs.remove(doc_id) {
            for token in doc.tokens {
                if let Some(ids) = self.tokens.get_mut(&token) {
                    ids.remove(doc_id);
                    if ids.is_empty() {
                        self.tokens.remove(&token);
                    }
                }
            }
        }
    }

    fn docs_for_token(&self, token: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        if let Some(ids) = self.tokens.get(token) {
            out.extend(ids.iter().cloned());
        }
        for (indexed, ids) in &self.tokens {
            if indexed.starts_with(token) {
                out.extend(ids.iter().cloned());
            }
        }
        out
    }
}

fn build_chat_summary_doc(chat: &Chat) -> Option<SearchDoc> {
    let last_message = chat
        .messages
        .last()
        .map(|msg| msg.content.clone())
        .unwrap_or_default();
    let display_name = chat_display_name(chat);
    let body = if last_message.is_empty() {
        chat.counterparty.clone()
    } else {
        format!("{} {}", chat.counterparty, last_message)
    };
    SearchDoc::new(
        format!("chat:{}:summary", chat.id),
        SearchResultKind::ChatSummary,
        Some(chat.id.clone()),
        None,
        None,
        None,
        display_name,
        truncate_text(&body, MAX_BODY_LEN),
        Some(chat.last_activity),
    )
}

fn build_chat_message_doc(chat: &Chat, msg: &ChatMessage) -> Option<SearchDoc> {
    let display_name = chat_display_name(chat);
    let body = format!("{} {}", chat.counterparty, msg.content);
    SearchDoc::new(
        format!("chat:{}:msg:{}", chat.id, msg.id),
        SearchResultKind::ChatMessage,
        Some(chat.id.clone()),
        None,
        Some(msg.id.clone()),
        None,
        display_name,
        truncate_text(&body, MAX_BODY_LEN),
        Some(msg.timestamp),
    )
}

fn chat_display_name(chat: &Chat) -> String {
    chat.counterparty_profile
        .as_ref()
        .map(|profile| profile.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| chat.counterparty.clone())
}

fn build_group_summary_doc(group_id: &GroupId, group: &Group) -> Option<SearchDoc> {
    let name = group
        .metadata
        .as_ref()
        .map(|meta| meta.name.clone())
        .unwrap_or_else(|| "Untitled group".to_string());
    let desc = group
        .metadata
        .as_ref()
        .and_then(|meta| meta.description.clone())
        .unwrap_or_default();
    let updated_at = group
        .metadata
        .as_ref()
        .map(|meta| meta.updated_at)
        .unwrap_or(0);
    SearchDoc::new(
        format!("group:{}:summary", group_id),
        SearchResultKind::GroupSummary,
        None,
        Some(group_id.clone()),
        None,
        None,
        name,
        truncate_text(&desc, MAX_BODY_LEN),
        Some(updated_at),
    )
}

fn build_group_message_doc(
    group_id: &GroupId,
    group: &Group,
    msg: &crate::crdt::MessageMeta,
) -> Option<SearchDoc> {
    let name = group
        .metadata
        .as_ref()
        .map(|meta| meta.name.clone())
        .unwrap_or_else(|| "Untitled group".to_string());
    SearchDoc::new(
        format!("group:{}:msg:{}", group_id, msg.message_id),
        SearchResultKind::GroupMessage,
        None,
        Some(group_id.clone()),
        Some(msg.message_id.clone()),
        Some(msg.thread_id.clone()),
        name,
        truncate_text(&msg.body, MAX_BODY_LEN),
        Some(msg.timestamp),
    )
}

fn doc_matches_scope(kind: SearchResultKind, scope: SearchScope) -> bool {
    match scope {
        SearchScope::All => true,
        SearchScope::Chats => matches!(
            kind,
            SearchResultKind::ChatSummary | SearchResultKind::ChatMessage
        ),
        SearchScope::Groups => matches!(
            kind,
            SearchResultKind::GroupSummary | SearchResultKind::GroupMessage
        ),
        SearchScope::Messages => matches!(
            kind,
            SearchResultKind::ChatMessage | SearchResultKind::GroupMessage
        ),
    }
}

fn score_doc(doc: &SearchDoc, query_tokens: &[String]) -> i64 {
    let mut score = 0i64;
    let title_lower = doc.title.to_lowercase();
    for token in query_tokens {
        if doc.tokens.contains(token) {
            score += 10;
        } else if doc.tokens.iter().any(|t| t.starts_with(token)) {
            score += 5;
        }
        if title_lower.contains(token) {
            score += 3;
        }
    }
    if matches!(
        doc.kind,
        SearchResultKind::ChatSummary | SearchResultKind::GroupSummary
    ) {
        score += 2;
    }
    score
}

fn doc_to_result(doc: &SearchDoc, query_tokens: &[String]) -> SearchResultItem {
    let snippet = if doc.body.is_empty() {
        None
    } else {
        build_snippet(&doc.body, query_tokens)
    };
    SearchResultItem {
        kind: doc.kind,
        chat_id: doc.chat_id.clone(),
        group_id: doc.group_id.clone(),
        message_id: doc.message_id.clone(),
        thread_id: doc.thread_id.clone(),
        title: doc.title.clone(),
        snippet,
        timestamp: doc.timestamp,
    }
}

fn build_snippet(text: &str, query_tokens: &[String]) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let mut best_pos: Option<(usize, usize)> = None;
    for token in query_tokens {
        if let Some(pos) = lower.find(token) {
            let char_pos = lower[..pos].chars().count();
            let token_chars = token.chars().count();
            let end_pos = char_pos + token_chars;
            best_pos = match best_pos {
                None => Some((char_pos, end_pos)),
                Some((best_start, best_end)) => {
                    if char_pos < best_start {
                        Some((char_pos, end_pos))
                    } else {
                        Some((best_start, best_end))
                    }
                }
            };
        }
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let (start, end) = match best_pos {
        Some((start, end)) => {
            let window = 40;
            let start = start.saturating_sub(window);
            let end = (end + window).min(total);
            (start, end)
        }
        None => {
            let end = total.min(80);
            (0, end)
        }
    };
    let mut snippet: String = chars[start..end].iter().collect();
    if start > 0 {
        snippet = format!("...{}", snippet);
    }
    if end < total {
        snippet.push_str("...");
    }
    Some(snippet)
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    text.chars().take(max_len).collect()
}

fn tokenize(text: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            buf.push(ch.to_ascii_lowercase());
        } else if !buf.is_empty() {
            if buf.len() >= 2 {
                tokens.insert(buf.clone());
            }
            buf.clear();
        }
    }
    if !buf.is_empty() && buf.len() >= 2 {
        tokens.insert(buf);
    }
    tokens
}

fn tokenize_query(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            buf.push(ch.to_ascii_lowercase());
        } else if !buf.is_empty() {
            if buf.len() >= 2 {
                tokens.push(buf.clone());
            }
            buf.clear();
        }
    }
    if !buf.is_empty() && buf.len() >= 2 {
        tokens.push(buf);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
