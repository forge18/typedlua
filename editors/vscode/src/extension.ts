import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
    console.log('TypedLua extension is now active');

    // Start the language server
    startLanguageServer();

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('typedlua.restartServer', async () => {
            await restartLanguageServer();
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('typedlua.showOutputChannel', () => {
            client?.outputChannel.show();
        })
    );
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

async function startLanguageServer() {
    const config = vscode.workspace.getConfiguration('typedlua');
    const serverPath = config.get<string>('server.path', 'typedlua-lsp');

    // Define the server options
    const serverOptions: ServerOptions = {
        command: serverPath,
        args: [],
        transport: TransportKind.stdio,
        options: {
            env: process.env
        }
    };

    // Options to control the language client
    const clientOptions: LanguageClientOptions = {
        // Register the server for TypedLua documents
        documentSelector: [
            { scheme: 'file', language: 'typedlua' },
            { scheme: 'untitled', language: 'typedlua' }
        ],
        synchronize: {
            // Notify the server about file changes to '.tl' files in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tl')
        },
        outputChannelName: 'TypedLua Language Server',
        traceOutputChannel: vscode.window.createOutputChannel('TypedLua Language Server Trace'),
        revealOutputChannelOn: 2, // RevealOutputChannelOn.Error
        initializationOptions: {
            // Pass configuration to the server
            checkOnSave: config.get('compiler.checkOnSave', true),
            strictNullChecks: config.get('compiler.strictNullChecks', true),
            formatEnable: config.get('format.enable', true),
            formatIndentSize: config.get('format.indentSize', 4),
            inlayHintsTypeHints: config.get('inlayHints.typeHints', true),
            inlayHintsParameterHints: config.get('inlayHints.parameterHints', true)
        }
    };

    // Create the language client
    client = new LanguageClient(
        'typedlua',
        'TypedLua Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client (and server)
    try {
        await client.start();
        vscode.window.showInformationMessage('TypedLua Language Server started successfully');
    } catch (error) {
        vscode.window.showErrorMessage(
            `Failed to start TypedLua Language Server: ${error}`
        );
        console.error('Failed to start language server:', error);
    }
}

async function restartLanguageServer() {
    if (client) {
        vscode.window.showInformationMessage('Restarting TypedLua Language Server...');
        await client.stop();
        client = undefined;
    }
    await startLanguageServer();
}
