mod document;
mod providers;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use document::DocumentManager;
use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, CodeActionResolveRequest, Completion, DocumentSymbolRequest, Formatting,
    GotoDefinition, HoverRequest, InlayHintRequest, PrepareRenameRequest, RangeFormatting,
    References, Rename, SelectionRangeRequest, SemanticTokensFullRequest, SignatureHelpRequest,
    WorkspaceSymbolRequest,
};
use lsp_types::{*, Uri};
use providers::{
    CodeActionsProvider, CompletionProvider, DefinitionProvider, DiagnosticsProvider,
    FoldingRangeProvider, FormattingProvider, HoverProvider, InlayHintsProvider,
    ReferencesProvider, RenameProvider, SelectionRangeProvider, SemanticTokensProvider,
    SignatureHelpProvider, SymbolsProvider,
};
use std::error::Error;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Initialize tracing for LSP server
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr) // LSP uses stdout for protocol, log to stderr
        .init();

    // Create the LSP connection
    let (connection, io_threads) = Connection::stdio();

    // Server capabilities
    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![
                ".".to_string(),
                ":".to_string(),
                "@".to_string(),
                "<".to_string(),
                "{".to_string(),
                "(".to_string(),
            ]),
            resolve_provider: Some(true),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Options(
            CodeActionOptions {
                code_action_kinds: Some(vec![
                    CodeActionKind::QUICKFIX,
                    CodeActionKind::REFACTOR,
                    CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                ]),
                resolve_provider: Some(false),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        )),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::CLASS,
                        SemanticTokenType::INTERFACE,
                        SemanticTokenType::ENUM,
                        SemanticTokenType::TYPE,
                        SemanticTokenType::PARAMETER,
                        SemanticTokenType::VARIABLE,
                        SemanticTokenType::PROPERTY,
                        SemanticTokenType::FUNCTION,
                        SemanticTokenType::METHOD,
                        SemanticTokenType::KEYWORD,
                        SemanticTokenType::COMMENT,
                        SemanticTokenType::STRING,
                        SemanticTokenType::NUMBER,
                    ],
                    token_modifiers: vec![
                        SemanticTokenModifier::DECLARATION,
                        SemanticTokenModifier::READONLY,
                        SemanticTokenModifier::STATIC,
                        SemanticTokenModifier::ABSTRACT,
                        SemanticTokenModifier::DEPRECATED,
                        SemanticTokenModifier::MODIFICATION,
                    ],
                },
                range: Some(true),
                full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                ..Default::default()
            },
        )),
        inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
            InlayHintOptions {
                resolve_provider: Some(false),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        ))),
        ..Default::default()
    })
    .unwrap();

    // Initialize the connection
    let initialization_params = connection.initialize(server_capabilities)?;
    let _params: InitializeParams = serde_json::from_value(initialization_params)?;

    // Run the main loop
    main_loop(connection, _params)?;

    // Wait for IO threads to finish
    io_threads.join()?;

    Ok(())
}

fn main_loop(connection: Connection, _params: InitializeParams) -> Result<()> {
    let mut document_manager = DocumentManager::new();
    let diagnostics_provider = DiagnosticsProvider::new();
    let completion_provider = CompletionProvider::new();
    let hover_provider = HoverProvider::new();
    let definition_provider = DefinitionProvider::new();
    let references_provider = ReferencesProvider::new();
    let rename_provider = RenameProvider::new();
    let symbols_provider = SymbolsProvider::new();
    let formatting_provider = FormattingProvider::new();
    let code_actions_provider = CodeActionsProvider::new();
    let signature_help_provider = SignatureHelpProvider::new();
    let inlay_hints_provider = InlayHintsProvider::new();
    let selection_range_provider = SelectionRangeProvider::new();
    let semantic_tokens_provider = SemanticTokensProvider::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                handle_request(
                    &connection,
                    req,
                    &document_manager,
                    &completion_provider,
                    &hover_provider,
                    &definition_provider,
                    &references_provider,
                    &rename_provider,
                    &symbols_provider,
                    &formatting_provider,
                    &code_actions_provider,
                    &signature_help_provider,
                    &inlay_hints_provider,
                    &selection_range_provider,
                    &semantic_tokens_provider,
                )?;
            }
            Message::Notification(not) => {
                handle_notification(
                    &connection,
                    not,
                    &mut document_manager,
                    &diagnostics_provider,
                )?;
            }
            Message::Response(_resp) => {
                // Client responses to our requests - we don't currently send any
            }
        }
    }

    Ok(())
}

fn handle_request(
    connection: &Connection,
    req: Request,
    document_manager: &DocumentManager,
    completion_provider: &CompletionProvider,
    hover_provider: &HoverProvider,
    definition_provider: &DefinitionProvider,
    references_provider: &ReferencesProvider,
    rename_provider: &RenameProvider,
    symbols_provider: &SymbolsProvider,
    formatting_provider: &FormattingProvider,
    code_actions_provider: &CodeActionsProvider,
    signature_help_provider: &SignatureHelpProvider,
    inlay_hints_provider: &InlayHintsProvider,
    selection_range_provider: &SelectionRangeProvider,
    semantic_tokens_provider: &SemanticTokensProvider,
) -> Result<()> {
    match cast_request::<Completion>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position.text_document.uri;
            let position = params.text_document_position.position;

            let result = document_manager
                .get(uri)
                .map(|doc| completion_provider.provide(doc, position))
                .map(CompletionResponse::Array);

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<HoverRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;

            let result = document_manager
                .get(uri)
                .and_then(|doc| hover_provider.provide(doc, position));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<GotoDefinition>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;

            let result = document_manager
                .get(uri)
                .and_then(|doc| definition_provider.provide(uri, doc, position));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<References>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position.text_document.uri;
            let position = params.text_document_position.position;
            let include_declaration = params.context.include_declaration;

            let result = document_manager.get(uri).and_then(|doc| {
                references_provider.provide(uri, doc, position, include_declaration)
            });

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<PrepareRenameRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let position = params.position;

            let result = document_manager
                .get(uri)
                .and_then(|doc| rename_provider.prepare(doc, position));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<Rename>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position.text_document.uri;
            let position = params.text_document_position.position;
            let new_name = &params.new_name;

            let result = document_manager
                .get(uri)
                .and_then(|doc| rename_provider.rename(uri, doc, position, new_name));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<DocumentSymbolRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;

            let result = document_manager
                .get(uri)
                .map(|doc| symbols_provider.provide(doc))
                .map(DocumentSymbolResponse::Nested);

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<WorkspaceSymbolRequest>(req.clone()) {
        Ok((id, params)) => {
            let symbols = symbols_provider.provide_workspace_symbols(&params.query);
            let response = Response::new_ok(id, Some(symbols));
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<Formatting>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let options = params.options;

            let result = document_manager
                .get(uri)
                .map(|doc| formatting_provider.format_document(doc, options));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<RangeFormatting>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let range = params.range;
            let options = params.options;

            let result = document_manager
                .get(uri)
                .map(|doc| formatting_provider.format_range(doc, range, options));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<CodeActionRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let range = params.range;
            let context = params.context;

            let result = document_manager
                .get(uri)
                .map(|doc| code_actions_provider.provide(uri, doc, range, context));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<SignatureHelpRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;

            let result = document_manager
                .get(uri)
                .and_then(|doc| signature_help_provider.provide(doc, position));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<InlayHintRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let range = params.range;

            let result = document_manager
                .get(uri)
                .map(|doc| inlay_hints_provider.provide(doc, range));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<SelectionRangeRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;
            let positions = params.positions;

            let result = document_manager
                .get(uri)
                .map(|doc| selection_range_provider.provide(doc, positions));

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(req) => req,
    };

    match cast_request::<SemanticTokensFullRequest>(req.clone()) {
        Ok((id, params)) => {
            let uri = &params.text_document.uri;

            let result = document_manager
                .get(uri)
                .map(|doc| semantic_tokens_provider.provide_full(doc))
                .map(SemanticTokensResult::Tokens);

            let response = Response::new_ok(id, result);
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
        Err(_req) => {
            // Unknown request, ignore
        }
    };

    Ok(())
}

fn handle_notification(
    connection: &Connection,
    not: Notification,
    document_manager: &mut DocumentManager,
    diagnostics_provider: &DiagnosticsProvider,
) -> Result<()> {
    match cast_notification::<DidOpenTextDocument>(not.clone()) {
        Ok(params) => {
            let uri = params.text_document.uri.clone();
            document_manager.open(params);
            publish_diagnostics(connection, &uri, document_manager, diagnostics_provider)?;
            return Ok(());
        }
        Err(not) => not,
    };

    match cast_notification::<DidChangeTextDocument>(not.clone()) {
        Ok(params) => {
            let uri = params.text_document.uri.clone();
            document_manager.change(params);
            publish_diagnostics(connection, &uri, document_manager, diagnostics_provider)?;
            return Ok(());
        }
        Err(not) => not,
    };

    match cast_notification::<DidSaveTextDocument>(not.clone()) {
        Ok(params) => {
            let uri = params.text_document.uri.clone();
            document_manager.save(params);
            publish_diagnostics(connection, &uri, document_manager, diagnostics_provider)?;
            return Ok(());
        }
        Err(not) => not,
    };

    match cast_notification::<DidCloseTextDocument>(not.clone()) {
        Ok(params) => {
            let uri = params.text_document.uri.clone();
            document_manager.close(params);
            // Clear diagnostics on close
            send_notification::<PublishDiagnostics>(
                connection,
                PublishDiagnosticsParams {
                    uri,
                    diagnostics: vec![],
                    version: None,
                },
            )?;
            return Ok(());
        }
        Err(_not) => {
            // Unknown notification, ignore
        }
    };

    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: &Uri,
    document_manager: &DocumentManager,
    diagnostics_provider: &DiagnosticsProvider,
) -> Result<()> {
    if let Some(document) = document_manager.get(uri) {
        let diagnostics = diagnostics_provider.provide(document);
        send_notification::<PublishDiagnostics>(
            connection,
            PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics,
                version: None,
            },
        )?;
    }
    Ok(())
}

fn cast_request<R>(req: Request) -> std::result::Result<(RequestId, R::Params), Request>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    match req.extract(R::METHOD) {
        Ok(params) => Ok(params),
        Err(ExtractError::MethodMismatch(req)) => Err(req),
        Err(ExtractError::JsonError { method, error }) => {
            tracing::error!("Failed to deserialize request {}: {}", method, error);
            Err(Request::new(RequestId::from(0), method.to_string(), serde_json::Value::Null))
        }
    }
}

fn cast_notification<N>(
    not: Notification,
) -> std::result::Result<N::Params, Notification>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    match not.extract(N::METHOD) {
        Ok(params) => Ok(params),
        Err(ExtractError::MethodMismatch(not)) => Err(not),
        Err(ExtractError::JsonError { method, error }) => {
            tracing::error!("Failed to deserialize notification {}: {}", method, error);
            Err(Notification::new(method.to_string(), serde_json::Value::Null))
        }
    }
}

fn send_notification<N>(connection: &Connection, params: N::Params) -> Result<()>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::Serialize,
{
    let not = Notification::new(N::METHOD.to_string(), params);
    connection.sender.send(Message::Notification(not))?;
    Ok(())
}
