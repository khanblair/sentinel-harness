//! Attachments: what the composer plus button added to a chat.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::{Attachment, AttachmentId, AttachmentKind, ChatId};

use crate::{Result, Store, StoreError, enum_from_text, enum_text, time_from_text, time_text};

const COLUMNS: &str = "id, chat_id, kind, name, path, text, size, sent, created_at";

type AttachmentRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    bool,
    String,
);

impl Store {
    pub fn insert_attachment(&self, attachment: &Attachment) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            &format!(
                "INSERT INTO attachments ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            params![
                attachment.id.as_str(),
                attachment.chat.as_str(),
                enum_text(&attachment.kind)?,
                attachment.name,
                attachment.path,
                attachment.text,
                i64::try_from(attachment.size).unwrap_or(i64::MAX),
                attachment.sent,
                time_text(attachment.created_at)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_attachment(&self, id: &AttachmentId) -> Result<Option<Attachment>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM attachments WHERE id = ?1"),
                params![id.as_str()],
                read_row,
            )
            .optional()?;
        row.map(attachment_from_row).transpose()
    }

    /// The attachments of a chat, oldest first. `unsent_only` leaves out those a message carried.
    pub fn list_attachments(&self, chat: &ChatId, unsent_only: bool) -> Result<Vec<Attachment>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {COLUMNS} FROM attachments WHERE chat_id = ?1 AND (?2 = 0 OR sent = 0) \
                 ORDER BY created_at, id"
            ))?;
            let mapped = statement.query_map(params![chat.as_str(), unsent_only], read_row)?;
            mapped.collect::<rusqlite::Result<Vec<AttachmentRow>>>()?
        };
        rows.into_iter().map(attachment_from_row).collect()
    }

    pub fn delete_attachment(&self, id: &AttachmentId) -> Result<()> {
        let connection = self.connection()?;
        let removed = connection.execute(
            "DELETE FROM attachments WHERE id = ?1",
            params![id.as_str()],
        )?;
        if removed == 0 {
            return Err(StoreError::NotFound(format!("attachment {id}")));
        }
        Ok(())
    }

    /// Mark attachments as carried by a message.
    pub fn mark_attachments_sent(&self, ids: &[AttachmentId]) -> Result<()> {
        let connection = self.connection()?;
        for id in ids {
            connection.execute(
                "UPDATE attachments SET sent = 1 WHERE id = ?1",
                params![id.as_str()],
            )?;
        }
        Ok(())
    }
}

fn read_row(row: &Row<'_>) -> rusqlite::Result<AttachmentRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn attachment_from_row(row: AttachmentRow) -> Result<Attachment> {
    let (id, chat, kind, name, path, text, size, sent, created_at) = row;
    let kind: AttachmentKind = enum_from_text(&kind)?;
    Ok(Attachment {
        id: AttachmentId::new(id),
        chat: ChatId::new(chat),
        kind,
        name,
        path,
        text,
        size: u64::try_from(size).unwrap_or(0),
        sent,
        created_at: time_from_text(&created_at)?,
    })
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn attachment(id: &str, chat: &str, kind: AttachmentKind) -> Attachment {
        Attachment {
            id: AttachmentId::new(id),
            chat: ChatId::new(chat),
            kind,
            name: format!("name-{id}"),
            path: Some("src/lib.rs".to_owned()),
            text: None,
            size: 12,
            sent: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn an_attachment_round_trips() {
        let store = Store::open_in_memory().unwrap();
        let item = attachment("a1", "c1", AttachmentKind::File);
        store.insert_attachment(&item).unwrap();
        assert_eq!(store.get_attachment(&item.id).unwrap(), Some(item));
    }

    #[test]
    fn unsent_only_leaves_out_sent_attachments() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_attachment(&attachment("a1", "c1", AttachmentKind::File))
            .unwrap();
        store
            .insert_attachment(&attachment("a2", "c1", AttachmentKind::Note))
            .unwrap();
        store
            .mark_attachments_sent(&[AttachmentId::new("a1")])
            .unwrap();
        let chat = ChatId::new("c1");
        assert_eq!(store.list_attachments(&chat, false).unwrap().len(), 2);
        let unsent = store.list_attachments(&chat, true).unwrap();
        assert_eq!(unsent.len(), 1);
        assert_eq!(unsent[0].id, AttachmentId::new("a2"));
    }

    #[test]
    fn deleting_a_missing_attachment_is_not_found() {
        let store = Store::open_in_memory().unwrap();
        let error = store
            .delete_attachment(&AttachmentId::new("none"))
            .unwrap_err();
        assert!(matches!(error, StoreError::NotFound(_)));
    }
}
