# Hayazip
**日本語** | [**English**](README.md)

🚀 **超高速・マルチスレッド・SIMD対応のZIPライブラリ (Rust & Python)**

`hayazip`（ハヤジップ）は、最新のハードウェア性能を引き出すために設計された超高速なZIPライブラリです。Memory Mapped I/Oによるゼロコピー読み取り、`libdeflater` による SIMD 最適化された圧縮・展開、および `rayon` による並列処理を組み合わせ、ZIP の作成と展開を高速に行えます。

## 主な特徴
- **ゼロコピーな解析:** `memmap2` を用いてZIPファイル全体をメモリにマップし、カーネル・ユーザー空間間の不要なコピーを回避します。
- **SIMD最適化された圧縮・展開:** `libdeflater` をバックエンドに採用し、AVX2, AVX-512, NEON といった各アーキテクチャの命令を活用します。
- **マルチスレッド並列処理:** `rayon` により、独立したファイルを並列に圧縮・展開します。
- **ハードウェアによる高速CRC32:** `crc32fast` を用いて、展開時の整合性検証のオーバーヘッドを最小限に抑えます。
- **低フットプリントなZIP生成:** 圧縮済みデータを一時ファイルへスプールし、アーカイブ全体をメモリに保持せずに書き込みます。
- **安全なパス展開:** エントリ名の区切りを正規化し、`..`、絶対パス、ドライブプレフィックスを書き込み前に拒否します。
- **事前検査 (preflight):** central directory 全体を先に検査し、安全な出力先パスを呼び出し側が確認できます。
- **クロスプラットフォームなPythonバインディング:** PyO3を用いて構築され、Pythonからも簡単に呼び出せるように設計されています。

## Pythonからの使い方 (Quick Start)

### インストール
`uv` または `pip` で PyPI からインストールできます。CPython 3.8+ 向けの `abi3` wheel を Linux / macOS / Windows 向けに公開し、あわせてソース配布物も公開します。
```bash
uv add hayazip
# または
pip install hayazip
```

### 使い方
ZIP の作成と展開をシンプルに行えます：
```python
import hayazip

source_dir = "project_files"
archive_path = "project_files.zip"
output_dir = "extracted_files"

hayazip.create_zip(source_dir, archive_path)
hayazip.extract_zip(archive_path, output_dir)
print("展開完了！")
```

すでに ZIP バイト列を持っている場合は、一時ファイルを作らずにそのまま事前検査と展開ができます：
```python
import hayazip

entries = hayazip.preflight_zip_bytes(pptx_bytes)
for entry in entries:
    print(entry["path"], entry["compress_type"])

hayazip.extract_zip_bytes(pptx_bytes, "workdir/unpacked")
```

## Rustからの使い方 (Quick Start)

`Cargo.toml` に追加します:
```toml
[dependencies]
hayazip = "0.3.0"
```

### 使い方
```rust
use hayazip::{create_zip, extract, extract_from_bytes, preflight};

fn main() {
    let source_dir = "project_files";
    let archive_path = "project_files.zip";
    let output_dir = "extracted_files";

    create_zip(source_dir, archive_path).expect("ZIP作成に失敗しました");

    if let Err(e) = extract(archive_path, output_dir) {
        eprintln!("エラーが発生しました: {}", e);
    } else {
        println!("展開が完了しました！");
    }

    let safe_entries = preflight(archive_path).expect("事前検査に失敗しました");
    println!("{} 件のエントリを検査しました", safe_entries.len());

    let archive_bytes = std::fs::read(archive_path).expect("読み込みに失敗しました");
    extract_from_bytes(&archive_bytes, "extracted_from_bytes")
        .expect("bytes からの展開に失敗しました");
}
```

## 展開時の安全性
`hayazip` はファイルやディレクトリを作る前に metadata-only の preflight を走らせます。この段階で次を確認します。

- 区切り文字の揺れを forward slash ベースへ正規化する
- `..`、絶対パス、Windows のドライブプレフィックスを拒否する
- `dir` と `dir/file.txt` のような重複・衝突パスを拒否する
- 各 entry の local header と payload 範囲が構造的に読めることを確認する

展開前に検査結果だけ欲しい場合は、Rust では `preflight` / `preflight_bytes`、Python では `preflight_zip` / `preflight_zip_bytes` を使ってください。

## 圧縮方式の対応状況
現在の展開対応:

- `0` (`Stored` / 無圧縮)
- `8` (`Deflate`)

現在のアーカイブ作成対応:

- ディレクトリ、シンボリックリンク、空ファイル、圧縮しても得をしないファイルは `Stored`
- サイズ削減できる通常ファイルは `Deflate`

現在未対応:

- `Deflate64` (`9`), `BZIP2` (`12`), `LZMA` (`14`), `PPMd` (`98`), `Zstandard` (`93`) を含む、上記以外の ZIP compression method
- 暗号化された ZIP entry

## 実装方針
`hayazip` は `libdeflater` による SIMD 対応 DEFLATE と `rayon` による並列ファイル処理を使います。ZIP 生成時は圧縮済みメンバーを一時スプールし、メモリ使用量を抑えながら複数コアを活用します。

## 現在のスコープ
公開されている書き込み API は現状 `create_zip` のみです。entry 順序、timestamp、compression method、external attributes を明示指定できる低レベル writer API はまだ公開していません。

## ソースからのビルド (Python環境向け)
ソースコードからローカルのPython環境にビルド・インストールする場合：
```bash
pip install maturin
maturin develop --release
```

## ライセンス
MIT
