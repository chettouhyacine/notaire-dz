//! Append-only audit log with an HMAC-chained hash per entry.
//!
//! Each entry stores `seq`, the previous entry's hash and `HMAC-SHA256(audit_key, canonical)`.
//! Editing any field, deleting an entry or reordering entries breaks verification at a known
//! sequence number. The daily head hash is printed and countersigned by the notary, which is what
//! makes tampering detectable even against someone who has the audit key (docs/03 T3, T4).

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::crypto::SecretKey;
use crate::error::{CoreError, Result};

type HmacSha256 = Hmac<Sha256>;

pub const GENESIS_HASH: [u8; 32] = [0u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub seq: u64,
    pub occurred_at_ms: i64,
    pub actor_id: String,
    /// Machine action code, e.g. `party.read`, `act.finalise`, `export.create`.
    pub action: String,
    /// Subject identifier (entity id), never the entity content.
    pub subject: String,
    /// Short, content-free detail (e.g. field name, reason code).
    pub detail: String,
    pub prev_hash: [u8; 32],
    pub hash: [u8; 32],
}

impl AuditEvent {
    fn canonical(&self) -> Vec<u8> {
        // Length-prefixed fields prevent ambiguity between e.g. ("ab","c") and ("a","bc").
        let mut buf = Vec::new();
        let mut push = |bytes: &[u8]| {
            buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            buf.extend_from_slice(bytes);
        };
        push(&self.seq.to_le_bytes());
        push(&self.occurred_at_ms.to_le_bytes());
        push(self.actor_id.as_bytes());
        push(self.action.as_bytes());
        push(self.subject.as_bytes());
        push(self.detail.as_bytes());
        push(&self.prev_hash);
        buf
    }
}

pub struct AuditLog {
    key: SecretKey,
}

impl AuditLog {
    pub fn new(key: SecretKey) -> Self {
        AuditLog { key }
    }

    fn compute_hash(&self, event: &AuditEvent) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(self.key.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(b"notaire:audit:v1");
        mac.update(&event.canonical());
        let out = mac.finalize().into_bytes();
        let mut tag = [0u8; 32];
        tag.copy_from_slice(&out);
        tag
    }

    /// Append an event after `previous` (or at genesis when `previous` is `None`).
    pub fn append(
        &self,
        previous: Option<&AuditEvent>,
        occurred_at_ms: i64,
        actor_id: &str,
        action: &str,
        subject: &str,
        detail: &str,
    ) -> Result<AuditEvent> {
        if action.is_empty() || actor_id.is_empty() {
            return Err(CoreError::Validation(
                "audit entries need an actor and an action".into(),
            ));
        }
        let (seq, prev_hash) = match previous {
            Some(p) => (p.seq + 1, p.hash),
            None => (1, GENESIS_HASH),
        };
        let mut event = AuditEvent {
            seq,
            occurred_at_ms,
            actor_id: actor_id.to_string(),
            action: action.to_string(),
            subject: subject.to_string(),
            detail: detail.to_string(),
            prev_hash,
            hash: [0u8; 32],
        };
        event.hash = self.compute_hash(&event);
        Ok(event)
    }

    /// Verify the whole chain. Returns the first broken sequence number.
    pub fn verify(&self, events: &[AuditEvent]) -> Result<()> {
        let mut expected_prev = GENESIS_HASH;
        let mut expected_seq = 1u64;
        for event in events {
            if event.seq != expected_seq {
                return Err(CoreError::AuditBroken { seq: event.seq });
            }
            if event.prev_hash != expected_prev {
                return Err(CoreError::AuditBroken { seq: event.seq });
            }
            if self.compute_hash(event) != event.hash {
                return Err(CoreError::AuditBroken { seq: event.seq });
            }
            expected_prev = event.hash;
            expected_seq += 1;
        }
        Ok(())
    }

    /// Head hash for the daily countersignature printout.
    pub fn head_hash(events: &[AuditEvent]) -> [u8; 32] {
        events.last().map(|e| e.hash).unwrap_or(GENESIS_HASH)
    }

    pub fn head_hash_hex(events: &[AuditEvent]) -> String {
        hex::encode(Self::head_hash(events))
    }

    /// A chain that was truncated is detected by comparing the stored head with the recomputed one.
    pub fn verify_against_head(&self, events: &[AuditEvent], stored_head: &[u8; 32]) -> Result<()> {
        self.verify(events)?;
        if &Self::head_hash(events) != stored_head {
            return Err(CoreError::AuditBroken {
                seq: events.last().map(|e| e.seq).unwrap_or(0),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> AuditLog {
        AuditLog::new(SecretKey::from_bytes([42u8; 32]))
    }

    fn chain(n: usize) -> Vec<AuditEvent> {
        let l = log();
        let mut events: Vec<AuditEvent> = Vec::new();
        for i in 0..n {
            let prev = events.last().cloned();
            let e = l
                .append(
                    prev.as_ref(),
                    1_700_000_000_000 + i as i64,
                    "user-notary-1",
                    "case.read",
                    &format!("case-{i}"),
                    "reason=consultation",
                )
                .unwrap();
            events.push(e);
        }
        events
    }

    #[test]
    fn chain_verifies() {
        let events = chain(25);
        assert!(log().verify(&events).is_ok());
    }

    #[test]
    fn tampering_with_payload_is_detected_at_that_seq() {
        let mut events = chain(10);
        events[4].detail = "reason=export".to_string();
        match log().verify(&events) {
            Err(CoreError::AuditBroken { seq }) => assert_eq!(seq, 5),
            other => panic!("expected broken chain, got {other:?}"),
        }
    }

    #[test]
    fn deleting_an_entry_is_detected() {
        let mut events = chain(10);
        events.remove(3);
        assert!(log().verify(&events).is_err());
    }

    #[test]
    fn reordering_entries_is_detected() {
        let mut events = chain(6);
        events.swap(2, 3);
        assert!(log().verify(&events).is_err());
    }

    #[test]
    fn truncation_is_detected_against_stored_head() {
        let events = chain(8);
        let head = AuditLog::head_hash(&events);
        let truncated = &events[..5];
        assert!(log().verify_against_head(truncated, &head).is_err());
        assert!(log().verify_against_head(&events, &head).is_ok());
    }

    #[test]
    fn different_audit_key_cannot_reproduce_hashes() {
        let events = chain(3);
        let other = AuditLog::new(SecretKey::from_bytes([7u8; 32]));
        assert!(other.verify(&events).is_err());
    }

    #[test]
    fn entries_require_actor_and_action() {
        assert!(log().append(None, 0, "", "case.read", "s", "").is_err());
        assert!(log().append(None, 0, "u", "", "s", "").is_err());
    }
}
