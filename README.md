# claude_token_display

Claude Max プランの使用枠 (5時間セッション %、週間 %、Sonnet 週間 %、各リセット時刻) を macOS のメニューバー / Windows のシステムトレイに常時表示する常駐アプリ。

<!-- アイコンを差し替えたらここにスクショを貼る -->

## ダウンロード

[Releases](https://github.com/pentane11/claude_token_display/releases) から OS 別バイナリを取得してください。

| OS | ファイル |
|---|---|
| macOS (Apple Silicon) | `*_aarch64.dmg` |
| macOS (Intel)         | `*_x64.dmg` |
| Windows               | `*_x64-setup.exe` (NSIS) または `*_x64_en-US.msi` |

## 前提

- **`claude` CLI でログイン済み**であること（Claude Code の OAuth トークンを読み取ります）
- 未ログインの場合: ターミナルで `claude` を起動するとブラウザでログインフローが始まります。完了したらこのアプリを起動

## 初回起動時の注意

### macOS

#### ⚠️ 「"token_display.app" は壊れているため開けません」と出る場合

未署名アプリのため Gatekeeper が誤判定しています（実際には壊れていません）。
**ターミナルで以下を一度実行**してください:

```sh
xattr -dr com.apple.quarantine /Applications/token_display.app
```

その後 Launchpad / Spotlight から起動できます。

#### Keychain アクセス許可

初回起動時に **「`/usr/bin/security` が "Claude Code-credentials" にアクセスしようとしています」** というダイアログが出ます。**「常に許可」** をクリックしてください（「許可」だと毎回出ます）。

### Windows

SmartScreen が出たら「詳細情報」→「実行」。

Windows では Claude Code の認証情報は通常 `%USERPROFILE%\.claude\.credentials.json` にあります。`CLAUDE_CONFIG_DIR` を設定している場合は、そのディレクトリ配下の `.credentials.json` を読みます。

## 使い方

メニューバー / システムトレイに `43%` のような現在のセッション使用率が出ます。クリックで詳細パネル（5時間 / 週間 / 週間Sonnet の利用率とリセット時刻）。

データは 5 分おきに自動更新されます。手動で更新したい場合はパネル右上の `⟳` ボタン、または右クリックメニューの "Refresh now"。

## 開発

```bash
git clone https://github.com/pentane11/claude_token_display.git
cd claude_token_display
npm install
npm run dev         # tauri dev
```

要件: Node.js (20+) と Rust toolchain (`rustup`)。

## ライセンス

未設定 (個人プロジェクト)。
