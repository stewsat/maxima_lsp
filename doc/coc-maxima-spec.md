# coc-maxima — Extension Specification

Build a [coc.nvim](https://github.com/neoclide/coc.nvim) extension that wraps the `maxima-lsp` LSP server.

## Overview

`coc-maxima` is an npm package that activates on Maxima files and launches `maxima-lsp` as a language server via coc.nvim's `LanguageClient`.

## Repository

https://github.com/stewsat/coc-maxima

## Dependencies

The `maxima-lsp` binary must be installed separately (built from https://github.com/stewsat/maxima_lsp).

## Project structure

```
coc-maxima/
├── package.json
├── tsconfig.json
├── webpack.config.js
├── src/
│   └── index.ts
├── lib/
│   └── index.js          (compiled output)
└── README.md
```

## package.json

- **name**: `coc-maxima`
- **version**: `0.1.0`
- **description**: `Maxima LSP extension for coc.nvim`
- **main**: `lib/index.js`
- **engines**: `{ "coc": "^0.0.82" }`
- **activationEvents**: `["onLanguage:maxima"]`
- **contributes.configuration**: define these settings:

| Key | Type | Default | Description |
|---|---|---|---|
| `maxima-lsp.server.path` | `string` | `"maxima-lsp"` | Path to the maxima-lsp binary |
| `maxima-lsp.trace.server` | `string` | `"off"` | Trace level (`"off"`, `"messages"`, `"verbose"`) |

- **scripts**:
  - `build`: compile TypeScript + webpack
  - `prepare`: `npm run build`
  - `watch`: watch mode for development

- **devDependencies**:
  - `@types/node` — latest
  - `typescript` — `^5.0`
  - `coc.nvim` — `^0.0.82` (peerDependency too)
  - `webpack` — `^5`
  - `ts-loader` — `^9`
  - `webpack-cli` — `^5`

## src/index.ts

### activate

```typescript
import { ExtensionContext, LanguageClient, LanguageClientOptions, ServerOptions, services, workspace } from 'coc.nvim';

export async function activate(context: ExtensionContext): Promise<void> {
  const config = workspace.getConfiguration('maxima-lsp');
  const command = config.get<string>('server.path', 'maxima-lsp');

  const serverOptions: ServerOptions = { command };
  const clientOptions: LanguageClientOptions = {
    documentSelector: ['maxima'],
    synchronize: {
      configurationSection: 'maxima-lsp',
    },
  };

  const client = new LanguageClient('maxima-lsp', 'Maxima LSP', serverOptions, clientOptions);
  context.subscriptions.push(client.start());
}
```

Key points:
- Use `services` to register the language client
- The `documentSelector` must be `['maxima']` (corresponds to the `maxima` filetype)
- `configurationSection: 'maxima-lsp'` connects the settings
- The server command defaults to `maxima-lsp` (assumed on PATH); user can override with `maxima-lsp.server.path`

### deactivate

If a `deactivate` function is exported, call `client.stop()` on all started clients. This is optional — coc.nvim handles cleanup.

## tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "lib",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "declaration": true,
    "sourceMap": true
  },
  "include": ["src"],
  "exclude": ["node_modules", "lib"]
}
```

## webpack.config.js

```javascript
const path = require('path');

module.exports = {
  target: 'node',
  entry: './src/index.ts',
  output: {
    path: path.resolve(__dirname, 'lib'),
    filename: 'index.js',
    libraryTarget: 'commonjs2',
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  module: {
    rules: [
      {
        test: /\.ts$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      },
    ],
  },
  externals: {
    'coc.nvim': 'commonjs coc.nvim',
  },
};
```

## .gitignore

```
node_modules/
lib/
*.js.map
```

## README.md

Include:

```markdown
# coc-maxima

Maxima LSP support for [coc.nvim](https://github.com/neoclide/coc.nvim).

## Install

In your vim/neovim:

```
:CocInstall coc-maxima
```

Or add to your `coc-settings.json`:

```json
{
  "languageserver": {
    "maxima": {
      "command": "maxima-lsp",
      "filetypes": ["maxima", "mac", "max", "mx"],
      "rootPatterns": [".git"]
    }
  }
}
```

Requires the `maxima-lsp` binary: https://github.com/stewsat/maxima_lsp

## Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `maxima-lsp.server.path` | `string` | `"maxima-lsp"` | Path to the maxima-lsp binary |
| `maxima-lsp.trace.server` | `string` | `"off"` | LSP trace level |

## Development

```bash
npm install
npm run build
```

Link locally: `npm link` then `:CocInstall /path/to/coc-maxima`
```

## LSP server capabilities

The `maxima-lsp` server returns these capabilities on initialize:

| Capability | Value |
|---|---|
| `textDocumentSync` | `Incremental` (2) |
| `semanticTokensProvider` | Full + range, legend with 8 token types |
| `documentSymbolProvider` | `true` |
| `completionProvider` | Trigger chars: `(`, `,` |
| `hoverProvider` | `true` |

## Filetype detection

Users must register the Maxima filetype in their vimrc:

```vim
au BufRead,BufNewFile *.mac,*.max,*.mx,*.maxima set filetype=maxima
```

The extension can optionally provide a `filetype` contribution or recommend the user add the above line.

## Publishing

```bash
npm run build
npm publish
```

After publishing, users install with `:CocInstall coc-maxima`.
