use line_index::LineIndex;

use tower_lsp::{LanguageServer, LspService, Server, jsonrpc, lsp_types::*};

use crate::parser::{Crate, Source};
use crate::state::State;
use crate::{diagnostic, typecheck};

pub async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new).finish();

    Server::new(stdin, stdout, socket).serve(service).await
}

pub struct Backend {
    pub _client: tower_lsp::Client,
    pub state: State,
}

impl Backend {
    pub fn new(_client: tower_lsp::Client) -> Self {
        Self {
            _client,
            state: State::new(),
        }
    }
}

pub fn get_module_name(url: &Url) -> &str {
    url.path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("unknown")
        .split('.')
        .next()
        .unwrap_or("unknown")
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),

                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: None,
                        inter_file_dependencies: false,
                        workspace_diagnostics: false,
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                    },
                )),

                ..Default::default()
            },
        })
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let url = params.text_document.uri;
        let text = params.text_document.text;

        let module_name = get_module_name(&url);

        let db = &self.state.get_db();

        let source = Source::new(db, text);

        self.state.add_file(module_name.into(), source)
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let url = params.text_document.uri;
        let text = params.content_changes.pop().unwrap().text;
        let module_name = get_module_name(&url);

        let db = &self.state.get_db();

        let source = Source::new(db, text);

        self.state.add_file(module_name.into(), source)
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> tower_lsp::jsonrpc::Result<DocumentDiagnosticReportResult> {
        let url = params.text_document.uri;
        let module_name = get_module_name(&url);

        let db = &self.state.get_db();
        let source = self.state.get_file(module_name).unwrap();
        let c = Crate::new(db, source);

        let _ = typecheck::check_crate(db, c);

        let diags: Vec<&diagnostic::Diagnostic> =
            typecheck::check_crate::accumulated::<diagnostic::Diagnostic>(db, c);

        let idx = LineIndex::new(source.text(db));

        let diags = diags
            .into_iter()
            .map(|d| d.as_lsp_diagnostic(&idx))
            .collect();

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items: diags,
                },
            }),
        ))
    }
}
