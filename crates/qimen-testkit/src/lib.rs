pub fn sample_onebot11_event() -> serde_json::Value {
    serde_json::json!({
        "time": 1,
        "self_id": 10001,
        "post_type": "message",
        "message_type": "private",
        "user_id": 20002,
        "message": "hello"
    })
}
