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

## Rustからの使い方 (Quick Start)

`Cargo.toml` に追加します:
```toml
[dependencies]
hayazip = "0.2.0"
```

### 使い方
```rust
use hayazip::{create_zip, extract};

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
}
```

## 実装方針
`hayazip` は `libdeflater` による SIMD 対応 DEFLATE と `rayon` による並列ファイル処理を使います。ZIP 生成時は圧縮済みメンバーを一時スプールし、メモリ使用量を抑えながら複数コアを活用します。

## ソースからのビルド (Python環境向け)
ソースコードからローカルのPython環境にビルド・インストールする場合：
```bash
pip install maturin
maturin develop --release
```

## ライセンス
MIT
