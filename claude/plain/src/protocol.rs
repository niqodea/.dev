//! Claude Code's side of the conversation, as `claude -p` carries it in its stream-json format:
//! the events read from what it writes, and the commands written back.
//! Ref: sdk.d.ts in the npm package @anthropic-ai/claude-agent-sdk, which types every message

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Map, Value, json};

/// The id of the settings request, the one request sent before the first message
const SETTINGS_REQUEST_ID: &str = "settings";

/// Something Claude Code reported.
pub enum Event {
    /// A piece of the reply Claude is writing.
    Text(String),
    /// The end of a block of the reply, whose pieces arrived as `Text`.
    TextEnd,
    ToolUse(ToolUse),
    ToolResult(ToolResult),
    /// A tool that waits for the user's answer before it runs.
    Ask(Ask),
    State(State),
    /// The end of a turn, with the reason Claude Code gives when the turn ended before its work
    /// was done.
    TurnEnd {
        failure: Option<String>,
    },
    /// Settings files Claude Code skipped, so none of their rules or hooks apply.
    SettingsErrors(Vec<SettingsError>),
    /// A request this client has no answer for. Claude Code waits on it until it is refused.
    Unsupported {
        request_id: String,
        subtype: String,
    },
}

/// Whether Claude works, waits for a message, or waits for the answer to an ask.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Running,
    RequiresAction,
}

pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Map<String, Value>,
    /// Whether a subagent of Claude makes the call, rather than Claude itself.
    pub is_subagent: bool,
}

pub struct ToolResult {
    pub tool_use_id: String,
    /// None when the result holds content but no text, like references to tools.
    pub text: Option<String>,
    pub is_error: bool,
    pub is_subagent: bool,
}

pub struct Ask {
    request_id: String,
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: Map<String, Value>,
    pub kind: AskKind,
}

pub enum AskKind {
    /// Whether the tool may run. Allowing it for good adds `rules`, empty when Claude Code
    /// offers none.
    Permission {
        reason: Option<String>,
        rules: Vec<Value>,
    },
    /// The questions of an `AskUserQuestion` call, which runs once all of them are answered.
    Questions(Vec<Question>),
    /// The plan of an `ExitPlanMode` call, in Markdown, which Claude carries out once the user
    /// approves it.
    Plan(String),
}

#[derive(Deserialize)]
pub struct Question {
    #[serde(rename = "question")]
    pub text: String,
    /// A label of a few words, which Claude Code shows next to the question.
    pub header: String,
    pub options: Vec<Choice>,
    // The tool's schema defaults it to false
    #[serde(rename = "multiSelect", default)]
    pub is_multi_select: bool,
}

#[derive(Deserialize)]
pub struct Choice {
    pub label: String,
    pub description: String,
    pub preview: Option<String>,
}

#[derive(Deserialize)]
pub struct SettingsError {
    pub file: Option<String>,
    /// The field that failed, empty when the whole file did.
    pub path: String,
    pub message: String,
}

pub enum Answer {
    /// Run the tool with `input`, and from now on allow what `rules` match.
    Allow {
        input: Map<String, Value>,
        rules: Vec<Value>,
    },
    /// Refuse the tool with `message`, which Claude reads. `interrupt` also ends the turn, so
    /// Claude waits for the next message.
    Deny { message: String, interrupt: bool },
}

pub enum Command<'a> {
    Message {
        id: &'a str,
        text: &'a str,
    },
    Answer(&'a Ask, Answer),
    Interrupt {
        request_id: String,
    },
    /// Has Claude Code make way for the message with this id, which it already holds, as the
    /// send-now key of Claude Code's terminal does: it moves what the running turn waits on to
    /// the background where it can, and stops the turn otherwise. It does nothing when no turn
    /// runs or the running turn already took the message in, so it never stops the turn that
    /// answers the message.
    ///
    /// Its fields are Claude Code's own, absent from the SDK's types. Claude Code announces them
    /// as `interrupt_send_now_v1` when it starts, and one without them takes the request as a
    /// plain interrupt.
    /// Ref: <https://code.claude.com/docs/en/changelog>, on send now moving running tools to the
    /// background
    SendNow {
        request_id: String,
        message_id: &'a str,
    },
    GetSettings,
    Refuse {
        request_id: &'a str,
        subtype: &'a str,
    },
}

impl Command<'_> {
    /// The line that carries the command to Claude Code.
    pub fn to_line(&self) -> String {
        let message = match self {
            Command::Message { id, text } => json!({
                "type": "user",
                "session_id": "",
                "uuid": id,
                "message": {"role": "user", "content": text},
                "parent_tool_use_id": null,
                // Claude Code takes a message without an origin as from no one, and makes way
                // while it works only for a person's message
                // Ref: SDKMessageOrigin in sdk.d.ts
                "origin": {"kind": "human"},
                // Claude Code runs a message that starts with / as a command, unless it comes
                // marked as composed by the client, which delivers it as written. The mark
                // also holds back the context Claude Code attaches to the turn until Claude's
                // first tool call, so only such a message carries it.
                "client_composed": text.starts_with('/'),
            }),
            Command::Answer(ask, answer) => {
                let result = match answer {
                    Answer::Allow { input, rules } => json!({
                        "behavior": "allow",
                        "updatedInput": input,
                        "updatedPermissions": rules,
                        "toolUseID": ask.tool_use_id,
                    }),
                    Answer::Deny { message, interrupt } => json!({
                        "behavior": "deny",
                        "message": message,
                        "interrupt": interrupt,
                        "toolUseID": ask.tool_use_id,
                    }),
                };
                json!({
                    "type": "control_response",
                    "response": {"subtype": "success", "request_id": ask.request_id, "response": result},
                })
            }
            Command::Interrupt { request_id } => json!({
                "type": "control_request",
                "request_id": request_id,
                "request": {"subtype": "interrupt"},
            }),
            Command::SendNow {
                request_id,
                message_id,
            } => json!({
                "type": "control_request",
                "request_id": request_id,
                "request": {"subtype": "interrupt", "send_now": true, "message_uuid": message_id},
            }),
            Command::GetSettings => json!({
                "type": "control_request",
                "request_id": SETTINGS_REQUEST_ID,
                "request": {"subtype": "get_settings"},
            }),
            Command::Refuse {
                request_id,
                subtype,
            } => json!({
                "type": "control_response",
                "response": {
                    "subtype": "error",
                    "request_id": request_id,
                    "error": format!("This client does not support {subtype}"),
                },
            }),
        };
        format!("{message}\n")
    }
}

/// Reads the events in one line Claude Code wrote, none for a message that reports nothing a
/// person reads or answers.
pub fn read_events(line: &str) -> Result<Vec<Event>, String> {
    let message: Message = serde_json::from_str(line)
        .map_err(|error| format!("Unreadable message from Claude Code: {error}: {line}"))?;
    let events = match message {
        Message::StreamEvent {
            event:
                StreamEvent::ContentBlockDelta {
                    delta: Delta::TextDelta { text },
                },
            parent_tool_use_id: None,
        } => vec![Event::Text(text)],
        // Claude Code sends each block whole once it has streamed it, so the text of a block
        // already arrived piece by piece
        Message::Assistant {
            message,
            parent_tool_use_id,
        } => message
            .content
            .into_blocks()
            .into_iter()
            .filter_map(|block| match block {
                Block::Text {} if parent_tool_use_id.is_none() => Some(Event::TextEnd),
                Block::ToolUse { id, name, input } => Some(Event::ToolUse(ToolUse {
                    id,
                    name,
                    input,
                    is_subagent: parent_tool_use_id.is_some(),
                })),
                Block::Text {} | Block::ToolResult { .. } | Block::Other => None,
            })
            .collect(),
        Message::User {
            message,
            parent_tool_use_id,
        } => message
            .content
            .into_blocks()
            .into_iter()
            .filter_map(|block| match block {
                Block::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => Some(Event::ToolResult(ToolResult {
                    tool_use_id,
                    text: content.map_or(Some(String::new()), ResultContent::into_text),
                    is_error,
                    is_subagent: parent_tool_use_id.is_some(),
                })),
                Block::Text {} | Block::ToolUse { .. } | Block::Other => None,
            })
            .collect(),
        Message::ControlRequest {
            request_id,
            request,
        } => vec![read_request(request_id, request)?],
        Message::ControlResponse {
            response:
                Reply::Success {
                    request_id,
                    response,
                },
        } if request_id == SETTINGS_REQUEST_ID => {
            let settings: Settings = serde_json::from_value(response.unwrap_or_default())
                .map_err(|error| format!("Unreadable settings from Claude Code: {error}"))?;
            if settings.errors.is_empty() {
                vec![]
            } else {
                vec![Event::SettingsErrors(settings.errors)]
            }
        }
        Message::ControlResponse {
            response: Reply::Error { request_id, error },
        } => return Err(format!("Claude Code refused request {request_id}: {error}")),
        Message::Result { subtype } if subtype == "success" => {
            vec![Event::TurnEnd { failure: None }]
        }
        Message::Result { subtype } => vec![Event::TurnEnd {
            failure: Some(subtype),
        }],
        Message::System(System::SessionStateChanged { state }) => vec![Event::State(state)],
        Message::StreamEvent { .. }
        // The other requests sent are interrupts, whose success needs no word
        | Message::ControlResponse {
            response: Reply::Success { .. },
        }
        | Message::System(System::Other)
        | Message::Other => vec![],
    };
    Ok(events)
}

fn read_request(request_id: String, request: Value) -> Result<Event, String> {
    let subtype = request["subtype"].as_str().unwrap_or_default().to_string();
    if subtype != "can_use_tool" {
        return Ok(Event::Unsupported {
            request_id,
            subtype,
        });
    }
    let request: CanUseTool = serde_json::from_value(request)
        .map_err(|error| format!("Unreadable permission request from Claude Code: {error}"))?;
    let kind = if request.tool_name == "AskUserQuestion" {
        let input: QuestionsInput = serde_json::from_value(Value::Object(request.input.clone()))
            .map_err(|error| format!("Unreadable questions from Claude: {error}"))?;
        AskKind::Questions(input.questions)
    } else if request.tool_name == "ExitPlanMode" {
        let input: PlanInput = serde_json::from_value(Value::Object(request.input.clone()))
            .map_err(|error| format!("Unreadable plan from Claude: {error}"))?;
        AskKind::Plan(input.plan)
    } else {
        // Claude Code marks the asks whose rule would allow more than the call itself
        let rules = if request.suppress_always_allow_rule {
            vec![]
        } else {
            request.permission_suggestions
        };
        AskKind::Permission {
            reason: request.decision_reason,
            rules,
        }
    };
    Ok(Event::Ask(Ask {
        request_id,
        tool_use_id: request.tool_use_id,
        tool_name: request.tool_name,
        input: request.input,
        kind,
    }))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Message {
    StreamEvent {
        event: StreamEvent,
        parent_tool_use_id: Option<String>,
    },
    Assistant {
        message: Body,
        parent_tool_use_id: Option<String>,
    },
    User {
        message: Body,
        parent_tool_use_id: Option<String>,
    },
    ControlRequest {
        request_id: String,
        request: Value,
    },
    ControlResponse {
        response: Reply,
    },
    Result {
        subtype: String,
    },
    System(System),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
enum System {
    // Sent only when CLAUDE_CODE_SDK_READS_SESSION_STATE is set
    SessionStateChanged {
        state: State,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    ContentBlockDelta {
        delta: Delta,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Delta {
    TextDelta {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Body {
    content: Content,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Blocks(Vec<Block>),
    // Tried after Blocks, since it takes any value
    Text(IgnoredAny),
}

impl Content {
    fn into_blocks(self) -> Vec<Block> {
        match self {
            Content::Blocks(blocks) => blocks,
            // A message of plain text holds no tool use or result
            Content::Text(_) => vec![],
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block {
    Text {},
    ToolUse {
        id: String,
        name: String,
        input: Map<String, Value>,
    },
    ToolResult {
        tool_use_id: String,
        content: Option<ResultContent>,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ResultContent {
    Text(String),
    Blocks(Vec<ResultBlock>),
}

impl ResultContent {
    /// The content's text, none when it holds blocks but none of them text.
    fn into_text(self) -> Option<String> {
        match self {
            ResultContent::Text(text) => Some(text),
            ResultContent::Blocks(blocks) => {
                let has_blocks = !blocks.is_empty();
                let texts: Vec<String> = blocks
                    .into_iter()
                    .filter_map(|block| match block {
                        ResultBlock::Text { text } => Some(text),
                        ResultBlock::Other => None,
                    })
                    .collect();
                if has_blocks && texts.is_empty() {
                    None
                } else {
                    Some(texts.join("\n"))
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResultBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
enum Reply {
    Success {
        request_id: String,
        response: Option<Value>,
    },
    Error {
        request_id: String,
        error: String,
    },
}

#[derive(Deserialize)]
struct Settings {
    // Claude Code leaves the list out when no file failed
    #[serde(default)]
    errors: Vec<SettingsError>,
}

#[derive(Deserialize)]
struct CanUseTool {
    tool_name: String,
    input: Map<String, Value>,
    tool_use_id: String,
    // Claude Code leaves the list out when it has no rule to offer
    #[serde(default)]
    permission_suggestions: Vec<Value>,
    decision_reason: Option<String>,
    // Claude Code leaves the flag out when the rules may be offered
    #[serde(default)]
    suppress_always_allow_rule: bool,
}

#[derive(Deserialize)]
struct QuestionsInput {
    questions: Vec<Question>,
}

#[derive(Deserialize)]
struct PlanInput {
    plan: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exit_plan_mode_request_asks_about_its_plan() {
        let request = serde_json::json!({
            "subtype": "can_use_tool",
            "tool_name": "ExitPlanMode",
            "input": {"plan": "# Plan", "planFilePath": "/plan.md"},
            "tool_use_id": "1",
        });
        let Ok(Event::Ask(ask)) = read_request("request-1".to_string(), request) else {
            panic!("an ask");
        };
        assert!(matches!(ask.kind, AskKind::Plan(plan) if plan == "# Plan"));
    }
}
