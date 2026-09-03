// 校验函数
// Validation functions
use serde::de::DeserializeOwned;
use serde_json::Value;

// 桥接通道载荷边界
// Bridge payload limits
const MAX_KEY_LEN: usize = 256;
const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_TITLE_LEN: usize = 256;
const MAX_MESSAGE_LEN: usize = 2000;
const MAX_EVENT_TYPE_LEN: usize = 128;

// payload 反序列化
// Payload deserialization
pub fn parse_payload<T: DeserializeOwned>(payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|e| format!("InvalidPayload: {e}"))
}

// 存储键校验
// Storage key validation
pub fn validate_key(key: &str) -> Result<(), String> {
    let n = key.chars().count();
    if n == 0 || n > MAX_KEY_LEN {
        Err(format!("InvalidPayload: key must be 1..={MAX_KEY_LEN} chars"))
    } else {
        Ok(())
    }
}

// 事件类型校验
// Event type validation
pub fn validate_event_type(event_type: &str) -> Result<(), String> {
    let n = event_type.chars().count();
    let valid = n > 0
        && n <= MAX_EVENT_TYPE_LEN
        && event_type
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "InvalidPayload: event type must be 1..={MAX_EVENT_TYPE_LEN} chars of [A-Za-z0-9._-]"
        ))
    }
}

// 日志消息校验
// Log message validation
pub fn validate_message(message: &str) -> Result<(), String> {
    let n = message.chars().count();
    if n == 0 || n > MAX_MESSAGE_LEN {
        Err(format!("InvalidPayload: message must be 1..={MAX_MESSAGE_LEN} chars"))
    } else {
        Ok(())
    }
}

// 窗口标题校验
// Window title validation
pub fn validate_title(title: &str) -> Result<(), String> {
    let n = title.chars().count();
    if n == 0 || n > MAX_TITLE_LEN {
        Err(format!("InvalidPayload: title must be 1..={MAX_TITLE_LEN} chars"))
    } else {
        Ok(())
    }
}

// 动作名校验
// Action name validation
pub fn validate_action(action: &str) -> Result<(), String> {
    let n = action.chars().count();
    let valid = n > 0
        && n <= MAX_EVENT_TYPE_LEN
        && action
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "InvalidPayload: action must be 1..={MAX_EVENT_TYPE_LEN} chars of [A-Za-z0-9._-]"
        ))
    }
}

// 导出常量供其他模块使用
// Export constants for other modules
pub const MAX_VALUE_BYTES_LIMIT: usize = MAX_VALUE_BYTES;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_key_bounds() {
        assert!(validate_key("counter").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key(&"k".repeat(257)).is_err());
        assert!(validate_key(&"k".repeat(256)).is_ok());
    }

    #[test]
    fn validate_event_type_charset_and_length() {
        assert!(validate_event_type("clipboard_changed.1").is_ok());
        assert!(validate_event_type("clipboard-changed.v2").is_ok());
        assert!(validate_event_type("").is_err());
        assert!(validate_event_type("bad type!").is_err());
        assert!(validate_event_type(&"e".repeat(129)).is_err());
        assert!(validate_event_type(&"e".repeat(128)).is_ok());
    }

    #[test]
    fn validate_message_and_title_bounds() {
        assert!(validate_message("hello").is_ok());
        assert!(validate_message("").is_err());
        assert!(validate_message(&"m".repeat(2001)).is_err());
        assert!(validate_title("Demo").is_ok());
        assert!(validate_title("").is_err());
        assert!(validate_title(&"t".repeat(257)).is_err());
    }
}
