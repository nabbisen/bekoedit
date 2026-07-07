//! AppState extension — store exports.

use crate::store::{AppState, StoreError};

impl AppState {
    // --- RFC-035: HTML export ---

    /// Exports the current document's sanitized HTML to `path` as a
    /// self-contained HTML file. Never overwrites without an explicit call;
    /// the caller chooses the output path.
    pub fn export_html(&self, path: &std::path::Path) -> Result<(), StoreError> {
        let session = self.session.as_ref().ok_or(StoreError::NoDocument)?;
        let title = session
            .path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bekoedit export".into());
        let body_html = session.preview_html();
        let full = format!(
            r#"<!doctype html>
    <html lang="en">
    <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title}</title>
    <style>
      body {{ max-width: 780px; margin: 2rem auto; padding: 0 1.5rem;
             font-family: system-ui, sans-serif; line-height: 1.65; color: #222; }}
      pre  {{ background: #f6f6f2; padding: .75em 1em; border-radius: 6px; overflow-x: auto; }}
      code {{ font-size: .92em; }}
      blockquote {{ border-left: 3px solid #ccc; margin: 0; padding-left: 1em; color: #555; }}
      table {{ border-collapse: collapse; }} td, th {{ border: 1px solid #ddd; padding: .3em .7em; }}
    </style>
    </head>
    <body>
    {body_html}
    </body>
    </html>
    "#,
        );
        bekoedit_fs::atomic_write(path, &full)
            .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
        Ok(())
    }
}
