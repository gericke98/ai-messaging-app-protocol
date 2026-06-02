use crate::identity::Identity;
use crate::protocol;
use crate::signing;
use anyhow::Result;
use futures_util::SinkExt;
use rand::{Rng, SeedableRng};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// The concrete WebSocket sender type.
type WsSender = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;

/// Send a streamed text response token-by-token over WebSocket.
///
/// Follows streaming.md lifecycle:
/// 1. StreamStart with 1 TextBlock layout
/// 2. StreamDelta events with 30-80ms delay per chunk
/// 3. StreamEnd with collapsed signed message
pub async fn send_streamed_response(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
    text: &str,
) -> Result<()> {
    let stream_id = protocol::new_message_id();
    let message_id = protocol::new_message_id();
    // Use StdRng (Send-safe) instead of ThreadRng which is !Send across await points
    let mut rng = rand::rngs::StdRng::from_entropy();

    // 1. Send StreamStart
    let start = protocol::new_notification(
        "ainm.stream.start",
        serde_json::json!({
            "stream_id": stream_id,
            "message_id": message_id,
            "from": identity.address,
            "thread_id": thread_id,
            "timestamp": protocol::now_rfc3339(),
            "blocks": [
                { "type": "text", "version": "1.0", "source": "agent" }
            ]
        }),
    );
    let start_text = serde_json::to_string(&start)?;
    ws_sender
        .send(Message::Text(start_text.into()))
        .await
        .map_err(|e| anyhow::anyhow!("send stream start: {}", e))?;

    // 2. Split text into word-sized chunks and send deltas
    let words: Vec<&str> = text.split_inclusive(' ').collect();
    let mut chunk = String::new();
    let mut word_count = 0;

    for word in &words {
        chunk.push_str(word);
        word_count += 1;

        // Send every 2-4 words as a delta
        let chunk_size = rng.gen_range(2..=4);
        if word_count >= chunk_size {
            let delta = protocol::new_notification(
                "ainm.stream.delta",
                serde_json::json!({
                    "stream_id": stream_id,
                    "block_index": 0,
                    "delta": chunk
                }),
            );
            let delta_text = serde_json::to_string(&delta)?;
            ws_sender
                .send(Message::Text(delta_text.into()))
                .await
                .map_err(|e| anyhow::anyhow!("send delta: {}", e))?;

            // Artificial delay: 30-80ms per chunk (per Pitfall 5)
            let delay_ms = rng.gen_range(30..=80);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;

            chunk.clear();
            word_count = 0;
        }
    }

    // Send any remaining text
    if !chunk.is_empty() {
        let delta = protocol::new_notification(
            "ainm.stream.delta",
            serde_json::json!({
                "stream_id": stream_id,
                "block_index": 0,
                "delta": chunk
            }),
        );
        let delta_text = serde_json::to_string(&delta)?;
        ws_sender
            .send(Message::Text(delta_text.into()))
            .await
            .map_err(|e| anyhow::anyhow!("send final delta: {}", e))?;
    }

    // 3. Construct collapsed message and sign it
    let mut collapsed = serde_json::json!({
        "id": message_id,
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [
            {
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": text,
                "fallback": text
            }
        ]
    });

    // Sign the collapsed message
    signing::sign_envelope(&identity.key_pair, &mut collapsed)?;

    // Send StreamEnd
    let end = protocol::new_notification(
        "ainm.stream.end",
        serde_json::json!({
            "stream_id": stream_id,
            "status": "completed",
            "collapsed_message": collapsed
        }),
    );
    let end_text = serde_json::to_string(&end)?;
    ws_sender
        .send(Message::Text(end_text.into()))
        .await
        .map_err(|e| anyhow::anyhow!("send stream end: {}", e))?;

    Ok(())
}
