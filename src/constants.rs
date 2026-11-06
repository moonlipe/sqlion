//! Values read from the DBeaver Java source (there is no compiled program: the target is source code).
//! Each constant names the file:line it was read from, relative to `plugins/`.
//! The same numbers are listed in docs/MECHANICS.md and docs/mechanics.json.

/// ModelPreferences.java:356 — resultset.maxrows
pub const RESULT_SET_MAX_ROWS: usize = 200;
/// ModelPreferences.java:377 — QMConstants.PROP_HISTORY_DAYS
pub const HISTORY_DAYS: u32 = 90;
/// ModelPreferences.java:378 — QMConstants.PROP_ENTRIES_PER_PAGE
pub const HISTORY_PAGE: usize = 200;
/// SQLConstants.java:31 — DEFAULT_STATEMENT_DELIMITER
pub const STATEMENT_DELIMITER: &str = ";";
/// DBConstants.java:71 — NULL_VALUE_LABEL
pub const NULL_LABEL: &str = "[NULL]";
/// ResultSetPreferences.java:26 — MAX_DEF_COLUMN_WIDTH (characters)
pub const MAX_DEF_COLUMN_WIDTH: usize = 30;
/// ResultSetPreferences.java:27 - DEFAULT_EDIT_UNDO_LEVEL
pub const UNDO_LEVELS: usize = 200;
/// DataEditorsPreferencesInitializer.java:48 - RS_EDIT_NEW_ROWS_AFTER: new rows go after the current row
pub const NEW_ROWS_AFTER: bool = true;
/// DataEditorsPreferencesInitializer.java:58 - RESULT_SET_AUTO_FETCH_NEXT_SEGMENT
pub const AUTO_FETCH_NEXT_SEGMENT: bool = true;
/// DataEditorsPreferencesInitializer.java:103 - RESULT_TEXT_MAX_COLUMN_SIZE
pub const TEXT_MAX_COLUMN_SIZE: usize = 255;
/// SQLEditorPreferencesInitializer.java:49 - SCRIPT_ERROR_HANDLING = STOP_ROLLBACK: a failing script stops and the open transaction is rolled back
pub const SCRIPT_ERROR_STOP_ROLLBACK: bool = true;
/// SQLEditorPreferencesInitializer.java:140 - RESULT_SET_MAX_TABS_PER_QUERY
pub const MAX_RESULT_TABS: usize = 20;
/// SQLEditorPreferencesInitializer.java:85,88 - ENABLE_AUTO_ACTIVATION, AUTO_ACTIVATION_DELAY (ms)
pub const AUTOCOMPLETE_AUTO_ACTIVATION: bool = true;
/// SQLEditorPreferencesInitializer.java:89 - INSERT_SINGLE_PROPOSALS_AUTO
pub const AUTOCOMPLETE_INSERT_SINGLE: bool = true;
/// SQLEditorPreferencesInitializer.java:104 - INSERT_SPACE_AFTER_PROPOSALS
pub const AUTOCOMPLETE_SPACE_AFTER: bool = true;
/// SQLEditorPreferencesInitializer.java:107 - PROPOSALS_MATCH_CONTAINS
pub const AUTOCOMPLETE_MATCH_CONTAINS: bool = true;
/// SQLEditorPreferencesInitializer.java:105 - PROPOSAL_SORT_ALPHABETICALLY
pub const AUTOCOMPLETE_SORT_ALPHA: bool = true;
/// SQLEditorPreferencesInitializer.java:119-121 - SQLEDITOR_CLOSE_SINGLE_QUOTES / DOUBLE_QUOTES / BRACKETS
pub const AUTO_CLOSE_PAIRS: bool = true;
/// SQLEditorPreferencesInitializer.java:126 - SQL_FORMAT_KEYWORD_CASE_AUTO: keyword case follows the text
pub const FORMAT_KEYWORD_CASE_AUTO: bool = true;
/// SQLEditorPreferencesInitializer.java:129 - SQL_FORMAT_ACTIVE_QUERY: without a selection only the current query is formatted
pub const FORMAT_ACTIVE_QUERY: bool = true;
/// ModelPreferences.java:370 — RESULT_SET_BINARY_STRING_MAX_LEN
pub const BINARY_STRING_MAX_LEN: usize = 32;
/// data.transfer/plugin.xml:62 — CSV importer columnTypeSamplesCount
pub const IMPORT_TYPE_SAMPLES: usize = 100;
/// connect timeout in seconds (own choice; DBeaver's validation timeout is 10000 ms, ModelPreferences.java:324)
pub const CONNECT_TIMEOUT_SECS: u64 = 10;
/// StreamConsumerSettings.java:125 — outputFilePattern
pub const EXPORT_FILE_PATTERN: &str = "${table}_${timestamp}";

/// Connection types, DBPConnectionType.java:44-101.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum ConnType {
    #[default]
    Dev,
    Test,
    Prod,
}

impl ConnType {
    pub const ALL: [ConnType; 3] = [ConnType::Dev, ConnType::Test, ConnType::Prod];
    pub fn label(&self) -> &'static str {
        match self {
            ConnType::Dev => "Desenvolvimento",
            ConnType::Test => "Teste",
            ConnType::Prod => "Produção",
        }
    }
    /// background colour (DBPConnectionType.java:48, 67, 86)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            ConnType::Dev => (255, 255, 255),
            ConnType::Test => (214, 250, 207),
            ConnType::Prod => (250, 207, 207),
        }
    }
    /// auto-commit by default (lines 51, 70, 89)
    pub fn auto_commit(&self) -> bool {
        !matches!(self, ConnType::Prod)
    }
    /// confirm before running statements that change data (lines 52, 71, 90)
    pub fn confirm_execute(&self) -> bool {
        matches!(self, ConnType::Prod)
    }
    /// confirm before saving edited rows (lines 53, 72, 91)
    pub fn confirm_data_change(&self) -> bool {
        !matches!(self, ConnType::Dev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_types_match_dbeaver_source() {
        // DBPConnectionType.java:51-53, 70-72, 89-91
        assert!(ConnType::Dev.auto_commit() && ConnType::Test.auto_commit() && !ConnType::Prod.auto_commit());
        assert!(!ConnType::Dev.confirm_execute() && !ConnType::Test.confirm_execute() && ConnType::Prod.confirm_execute());
        assert!(!ConnType::Dev.confirm_data_change() && ConnType::Test.confirm_data_change() && ConnType::Prod.confirm_data_change());
        assert_eq!(ConnType::Test.color(), (214, 250, 207));
        assert_eq!(ConnType::Prod.color(), (250, 207, 207));
    }
}
