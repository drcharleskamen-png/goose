use crate::conversation::message::MessageContent;
use crate::session::session_manager::SessionType;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

/// Upper bound on matching messages fetched from SQLite before ranking in Rust.
/// Decoupled from the caller's `limit`, which bounds returned sessions.
const CANDIDATE_POOL_SIZE: i64 = 500;

/// Common words filtered from queries so they don't match nearly everything.
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "was", "were", "what", "when", "where", "which", "who", "whom",
    "how", "why", "did", "does", "done", "you", "your", "yours", "our", "ours", "this", "that",
    "these", "those", "with", "from", "have", "has", "had", "can", "could", "should", "would",
    "will", "shall", "about", "into", "out", "off", "over", "under", "than", "then", "them",
    "they", "their", "there", "here", "some", "any", "all", "not", "but", "use", "used", "using",
    "get", "got", "set", "let", "remind", "remember", "recall", "tell", "show", "find",
];

#[derive(Debug, Clone, Serialize)]
pub struct ChatRecallResult {
    pub session_id: String,
    pub session_description: String,
    pub session_working_dir: String,
    pub last_activity: DateTime<Utc>,
    pub total_messages_in_session: usize,
    pub messages: Vec<ChatRecallMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRecallMessage {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ChatRecallResults {
    pub results: Vec<ChatRecallResult>,
    pub total_matches: usize,
}

type SqlQueryRow = (
    String,
    String,
    String,
    DateTime<Utc>,
    String,
    String,
    DateTime<Utc>,
);

type SessionMessageGroup = (
    String,
    String,
    DateTime<Utc>,
    Vec<(String, String, DateTime<Utc>)>,
);

pub struct ChatHistorySearch<'a> {
    pool: &'a Pool<Sqlite>,
    query: &'a str,
    limit: usize,
    after_date: Option<DateTime<Utc>>,
    before_date: Option<DateTime<Utc>>,
    exclude_session_id: Option<String>,
    session_types: Vec<SessionType>,
}

impl<'a> ChatHistorySearch<'a> {
    pub fn new(
        pool: &'a Pool<Sqlite>,
        query: &'a str,
        limit: Option<usize>,
        after_date: Option<DateTime<Utc>>,
        before_date: Option<DateTime<Utc>>,
        exclude_session_id: Option<String>,
        session_types: Vec<SessionType>,
    ) -> Self {
        Self {
            pool,
            query,
            limit: limit.unwrap_or(10),
            after_date,
            before_date,
            exclude_session_id,
            session_types,
        }
    }

    pub async fn execute(self) -> Result<ChatRecallResults> {
        let keywords = self.parse_keywords();
        if keywords.is_empty() {
            return Ok(ChatRecallResults {
                results: vec![],
                total_matches: 0,
            });
        }

        let rows = self.fetch_rows(&keywords).await?;
        let session_messages = self.process_rows(rows);
        let session_totals = self.get_session_totals(&session_messages).await?;
        let results = self.convert_to_results(session_messages, session_totals);

        Ok(results)
    }

    async fn fetch_rows(&self, keywords: &[String]) -> Result<Vec<SqlQueryRow>> {
        let sql = self.build_sql(keywords);
        let mut query_builder = sqlx::query_as::<_, SqlQueryRow>(&sql);

        for keyword in keywords {
            query_builder = query_builder.bind(format!("%{keyword}%"));
        }

        if let Some(exclude_id) = &self.exclude_session_id {
            query_builder = query_builder.bind(exclude_id);
        }

        for t in &self.session_types {
            query_builder = query_builder.bind(t.to_string());
        }

        if let Some(after) = self.after_date {
            query_builder = query_builder.bind(after);
        }
        if let Some(before) = self.before_date {
            query_builder = query_builder.bind(before);
        }

        // Fetch a generous candidate pool of matching messages, then rank in Rust.
        // `limit` bounds the number of returned sessions, not raw rows.
        query_builder = query_builder.bind(CANDIDATE_POOL_SIZE);

        Ok(query_builder.fetch_all(self.pool).await?)
    }

    fn parse_keywords(&self) -> Vec<String> {
        let cleaned: Vec<String> = self
            .query
            .split_whitespace()
            .map(|word| {
                word.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|word| !word.is_empty())
            .collect();

        let mut keywords: Vec<String> = cleaned
            .iter()
            .filter(|word| word.len() >= 3 && !STOPWORDS.contains(&word.as_str()))
            .cloned()
            .collect();

        // If filtering removed everything (e.g. a very short query), fall back to
        // the cleaned words so we still search rather than return nothing.
        if keywords.is_empty() {
            keywords = cleaned;
        }

        keywords.sort();
        keywords.dedup();
        keywords
    }

    fn build_sql(&self, keywords: &[String]) -> String {
        let mut sql = String::from(
            r#"
            SELECT 
                s.id as session_id,
                s.description as session_description,
                s.working_dir as session_working_dir,
                s.created_at as session_created_at,
                m.role,
                m.content_json,
                m.timestamp
            FROM messages m
            INNER JOIN sessions s ON m.session_id = s.id
            WHERE EXISTS (
                SELECT 1 FROM json_each(m.content_json) 
                WHERE json_extract(value, '$.type') = 'text' 
                AND (
        "#,
        );

        for (i, _) in keywords.iter().enumerate() {
            if i > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str("LOWER(json_extract(value, '$.text')) LIKE ?");
        }

        sql.push_str(
            r#"
                )
            )
        "#,
        );

        if self.exclude_session_id.is_some() {
            sql.push_str(" AND s.id != ?");
        }

        if !self.session_types.is_empty() {
            let placeholders: String = self
                .session_types
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND s.session_type IN ({})", placeholders));
        }

        if self.after_date.is_some() {
            sql.push_str(" AND m.timestamp >= ?");
        }
        if self.before_date.is_some() {
            sql.push_str(" AND m.timestamp <= ?");
        }

        sql.push_str(" ORDER BY m.timestamp DESC LIMIT ?");

        sql
    }

    fn process_rows(&self, rows: Vec<SqlQueryRow>) -> HashMap<String, SessionMessageGroup> {
        let mut session_messages: HashMap<String, SessionMessageGroup> = HashMap::new();

        for (
            session_id,
            session_description,
            session_working_dir,
            session_created_at,
            role,
            content_json,
            timestamp,
        ) in rows
        {
            if let Ok(content_vec) = serde_json::from_str::<Vec<MessageContent>>(&content_json) {
                let text_parts = Self::extract_text_content(content_vec);

                if !text_parts.is_empty() {
                    let entry = session_messages.entry(session_id.clone()).or_insert((
                        session_description.clone(),
                        session_working_dir.clone(),
                        session_created_at,
                        Vec::new(),
                    ));
                    entry
                        .3
                        .push((role.clone(), text_parts.join("\n"), timestamp));
                }
            }
        }

        session_messages
    }

    fn extract_text_content(content_vec: Vec<MessageContent>) -> Vec<String> {
        content_vec
            .into_iter()
            .filter_map(|content| match content {
                MessageContent::Text(ref tc) => Some(tc.text.clone()),
                MessageContent::ToolRequest(ref tr) => {
                    Some(format!("[Tool: {}]", tr.to_readable_string()))
                }
                MessageContent::ToolResponse(_) => Some("[Tool Response]".to_string()),
                MessageContent::Thinking(ref t) => Some(format!("[Thinking: {}]", t.thinking)),
                _ => None,
            })
            .collect()
    }

    async fn get_session_totals(
        &self,
        session_messages: &HashMap<String, SessionMessageGroup>,
    ) -> Result<HashMap<String, usize>> {
        let mut session_totals: HashMap<String, usize> = HashMap::new();
        for session_id in session_messages.keys() {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE session_id = ?")
                    .bind(session_id)
                    .fetch_one(self.pool)
                    .await
                    .unwrap_or(0);
            session_totals.insert(session_id.clone(), count as usize);
        }
        Ok(session_totals)
    }

    fn convert_to_results(
        &self,
        session_messages: HashMap<String, SessionMessageGroup>,
        session_totals: HashMap<String, usize>,
    ) -> ChatRecallResults {
        let keywords = self.parse_keywords();

        let mut scored: Vec<(SessionScore, ChatRecallResult)> = session_messages
            .into_iter()
            .map(
                |(session_id, (description, working_dir, _created_at, messages))| {
                    // Rank messages within the session by how well each matches,
                    // so the most relevant message leads (and becomes the snippet).
                    let mut message_vec: Vec<ChatRecallMessage> = messages
                        .into_iter()
                        .map(|(role, content, timestamp)| ChatRecallMessage {
                            role,
                            content,
                            timestamp,
                        })
                        .collect();

                    message_vec.sort_by(|a, b| {
                        message_relevance(&b.content, &keywords)
                            .cmp(&message_relevance(&a.content, &keywords))
                            .then_with(|| b.timestamp.cmp(&a.timestamp))
                    });

                    let last_activity = message_vec
                        .iter()
                        .map(|m| m.timestamp)
                        .max()
                        .unwrap_or_else(chrono::Utc::now);

                    let total_messages_in_session =
                        session_totals.get(&session_id).copied().unwrap_or(0);

                    let score = SessionScore::compute(&message_vec, &description, &keywords);

                    let result = ChatRecallResult {
                        session_id,
                        session_description: description,
                        session_working_dir: working_dir,
                        last_activity,
                        total_messages_in_session,
                        messages: message_vec,
                    };
                    (score, result)
                },
            )
            .collect();

        // Rank sessions: keyword coverage first, then total matches, then recency.
        scored.sort_by(|a, b| {
            b.0.coverage
                .cmp(&a.0.coverage)
                .then_with(|| b.0.total_hits.cmp(&a.0.total_hits))
                .then_with(|| b.1.last_activity.cmp(&a.1.last_activity))
        });

        let mut results: Vec<ChatRecallResult> = scored
            .into_iter()
            .take(self.limit)
            .map(|(_, result)| result)
            .collect();

        // Trim each session's message list to the most relevant few so the
        // snippet/leading messages are useful rather than the whole transcript.
        for result in &mut results {
            result.messages.truncate(MAX_MESSAGES_PER_SESSION);
        }

        let total_matches = results.iter().map(|r| r.messages.len()).sum();
        ChatRecallResults {
            results,
            total_matches,
        }
    }
}

const MAX_MESSAGES_PER_SESSION: usize = 3;

/// Markers identifying compaction/continuation summaries. These messages recap a
/// whole session, so they are keyword-dense and match almost any query — they
/// should not outrank a message that actually discusses the topic.
const SUMMARY_MARKERS: &[&str] = &[
    "<analysis>",
    "your context was compacted",
    "the previous message contains a summary of the conversation",
];

fn is_summary_message(content: &str) -> bool {
    let lower = content.to_lowercase();
    SUMMARY_MARKERS.iter().any(|m| lower.contains(m))
}

/// Relevance of a single message: distinct keywords matched (weighted heavily)
/// plus total keyword occurrences. Compaction summaries are penalized so a
/// message genuinely about the topic wins over a session recap.
fn message_relevance(content: &str, keywords: &[String]) -> usize {
    let lower = content.to_lowercase();
    let distinct = keywords.iter().filter(|k| lower.contains(*k)).count();
    let occurrences: usize = keywords.iter().map(|k| lower.matches(k).count()).sum();
    let score = distinct * 10 + occurrences;
    if is_summary_message(content) {
        score / 4
    } else {
        score
    }
}

struct SessionScore {
    /// Number of distinct query keywords found anywhere in the session.
    coverage: usize,
    /// Total keyword occurrences across the session's matched messages.
    total_hits: usize,
}

impl SessionScore {
    fn compute(messages: &[ChatRecallMessage], description: &str, keywords: &[String]) -> Self {
        // Coverage and hits are computed over non-summary messages so a session
        // that only matches inside a compaction recap doesn't rank as on-topic.
        let mut haystack = description.to_lowercase();
        for m in messages {
            if is_summary_message(&m.content) {
                continue;
            }
            haystack.push(' ');
            haystack.push_str(&m.content.to_lowercase());
        }

        let mut coverage = keywords.iter().filter(|k| haystack.contains(*k)).count();
        let mut total_hits = keywords.iter().map(|k| haystack.matches(k).count()).sum();

        // Fall back to summary content if that's all a session has, but at a
        // discount so genuine matches elsewhere win.
        if coverage == 0 {
            let summary: String = messages
                .iter()
                .filter(|m| is_summary_message(&m.content))
                .map(|m| m.content.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            coverage = keywords.iter().filter(|k| summary.contains(*k)).count();
            total_hits = keywords
                .iter()
                .map(|k| summary.matches(k).count())
                .sum::<usize>()
                / 4;
        }

        Self {
            coverage,
            total_hits,
        }
    }
}
