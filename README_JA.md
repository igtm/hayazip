# Hayazip
**日本語** | [**English**](README.md)

🚀 **超高速・マルチスレッド・SIMD対応のZIP展開ライブラリ (Rust & Python)**

`hayazip`（ハヤジップ）は、最新のハードウェア性能を限界まで引き出すためにゼロから設計された超高速なZIP展開ライブラリです。Memory Mapped I/Oによるゼロコピー読み取り、`libdeflater`によるSIMD最適化された展開、および`rayon`によるスレッドプールを用いた並列抽出処理を組み合わせることで、標準のUnix `unzip` コマンドと比較して **最大10倍の高速化** を実現しています。

## 主な特徴
- **ゼロコピーな解析:** `memmap2` を用いてZIPファイル全体をメモリにマップし、カーネル・ユーザー空間間の不要なコピーを回避します。
- **SIMD最適化された展開:** `libdeflater` をバックエンドに採用し、AVX2, AVX-512, NEON といった各アーキテクチャの命令を利用した超高速なDeflate展開を行います。
- **マルチスレッド完全並列展開:** `rayon` のFork-Joinモデルを用いて、互いに依存関係のないアーカイブ内の各ファイルを全CPUコアをフル活用して並行処理します。
- **ハードウェアによる高速CRC32:** `crc32fast` を用いて、展開時の整合性検証のオーバーヘッドを最小限に抑えます。
- **クロスプラットフォームなPythonバインディング:** PyO3を用いて構築され、Pythonからも簡単に呼び出せるように設計されています。

## Pythonからの使い方 (Quick Start)

### インストール
PyPIからインストール可能です（Linux, macOS, Windows用のビルド済みWheelが提供されます）。
```bash
pip install hayazip
```

### 使い方
標準ライブラリの `zipfile` モジュールよりも圧倒的に高速に展開できます：
```python
import hayazip

archive_path = "huge_archive.zip"
output_dir = "extracted_files"

# CPUの全コアを使用してアーカイブを展開します
hayazip.extract_zip(archive_path, output_dir)
print("展開完了！")
```

## Rustからの使い方 (Quick Start)

`Cargo.toml` に追加します:
```toml
[dependencies]
hayazip = "0.1.0"
```

### 使い方
```rust
use hayazip::extract;

fn main() {
    let archive_path = "huge_archive.zip";
    let output_dir = "extracted_files";

    if let Err(e) = extract(archive_path, output_dir) {
        eprintln!("エラーが発生しました: {}", e);
    } else {
        println!("展開が完了しました！");
    }
}
```

## ベンチマーク
10個の5MBファイル（合計50MB）が圧縮されたZIPファイルを用いた展開テスト:
- `unzip` (Unix標準コマンド): 約162ms
- `hayazip`: **約16ms (およそ10倍の高速化)**

## ソースからのビルド (Python環境向け)
ソースコードからローカルのPython環境にビルド・インストールする場合：
```bash
pip install maturin
maturin develop --release
```

## ライセンス
MIT
