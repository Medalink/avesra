//! Bytes remain private to the actual read worker until semantic completion.
use super::{Batch, Mailbox};
use avesra_contracts::{
    ErrorCode,
    browser::{mailbox as wire, reading::Context},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::Instant;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    batches: Vec<Batch>,
}

pub struct Stream {
    context: Context,
    account: String,
    requested: u16,
    deadline: Instant,
    bytes: String,
    chunks: u16,
    digest: String,
    finished: bool,
}
impl Stream {
    pub(crate) fn new(
        context: Context,
        account: String,
        requested: u16,
        deadline: Instant,
    ) -> Self {
        Self {
            context,
            account,
            requested,
            deadline,
            bytes: String::new(),
            chunks: 0,
            digest: wire::EMPTY_DIGEST.into(),
            finished: false,
        }
    }
    pub(crate) fn append(&mut self, chunk: wire::Chunk) -> Result<wire::Ack, ErrorCode> {
        chunk.validate()?;
        if self.finished
            || Instant::now() >= self.deadline
            || chunk.context != self.context
            || chunk.ordinal != self.chunks
            || chunk.previous != self.digest
            || self
                .bytes
                .len()
                .checked_add(chunk.data.len())
                .is_none_or(|n| n > wire::STREAM_BYTES)
        {
            return Err(ErrorCode::Stale);
        }
        let mut hash = Sha256::new();
        hash.update(self.digest.as_bytes());
        hash.update(b"\n");
        hash.update(self.chunks.to_string().as_bytes());
        hash.update(b"\n");
        hash.update(chunk.data.as_bytes());
        self.digest = format!("{:x}", hash.finalize());
        self.bytes.push_str(&chunk.data);
        self.chunks += 1;
        if Instant::now() >= self.deadline {
            self.bytes.clear();
            return Err(ErrorCode::Expired);
        }
        Ok(wire::Ack {
            context: self.context.clone(),
            ordinal: chunk.ordinal,
            digest: self.digest.clone(),
        })
    }
    pub(crate) fn finish(
        &mut self,
        terminal: &wire::Terminal,
    ) -> Result<Option<Mailbox>, ErrorCode> {
        terminal.validate()?;
        if self.finished
            || Instant::now() >= self.deadline
            || terminal.account != self.account
            || terminal.chunks != self.chunks
            || terminal.bytes as usize != self.bytes.len()
            || terminal.digest != self.digest
        {
            return Err(ErrorCode::Stale);
        }
        self.finished = true;
        let bytes = std::mem::take(&mut self.bytes);
        if terminal.incomplete.is_some() {
            return Ok(None);
        }
        let document: Document = serde_json::from_str(&bytes).map_err(|_| ErrorCode::Malformed)?;
        if document.batches.is_empty() || document.batches.len() > 32 {
            return Err(ErrorCode::Malformed);
        }
        let mut mailbox = Mailbox::new(
            self.context.scope.revision.uuid(),
            self.account.clone(),
            self.requested,
            self.deadline,
        )?;
        let mut revision = 0;
        for batch in document.batches {
            if batch.document != terminal.document.document.uuid().to_string()
                || batch.dom_revision > terminal.dom_revision
                || batch.dom_revision < revision
            {
                return Err(ErrorCode::Stale);
            }
            revision = batch.dom_revision;
            mailbox.append(batch)?;
        }
        mailbox.actual_count()?;
        Ok(Some(mailbox))
    }
}
