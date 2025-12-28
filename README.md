# 🏹 BTC Hunter: High-Performance Bitcoin Brute-Forcer (Pure Rust)

> **WARNING:** This tool is for educational and research purposes only. Validating key ownership on the Bitcoin network without authorization may be illegal in your jurisdiction.

## 🚀 Overview

BTC Hunter is an ultra-high-performance, multi-threaded Bitcoin Private Key scanner written in **Pure Rust**. It is designed to verify **millions of keys per second** using advanced elliptic curve optimization and a hybrid offline/online verification architecture.

### ⚡ Key Features
- **Speed**: Scans **~1.8 Million Keys/Second** on standard hardware (M2 Air).
- **Engine**: Pure Rust utilizing `secp256k1` C-bindings for maximum efficiency.
- **Database**: Loads **57.7 Million** funded addresses into RAM for **O(1)** instant lookups.
- **Hybrid Logic**:
  - **Tier 1 (Offline)**: Checks generated keys against local RAM database (Zero Latency).
  - **Tier 2 (Online)**: Only queries APIs if an Offline match is found (Anti-Ban).
- **Automation**: "One-Click" setup script (`./hunter`) handles dependencies, compilation, and database syncing.
- **Live UI**: Real-time terminal dashboard with speed, total checked, and discovery metrics.

---

## 🛠 Architecture & Technical Details

### 1. Elliptic Curve Arithmetic
- **Library**: Uses `rust-secp256k1` (bindings to `libsecp256k1`).
- **Optimization**: "Key Walking". instead of generating a random key every time ($G \times k$), we generate one random start point and then add $G$ repeatedly ($P_{n+1} = P_n + G$). This avoids expensive scalar multiplication, reducing operations to simple point addition.

### 2. Memory Management (The "Big RAM" Strategy)
- **Data Structure**: `HashSet<[u8; 20]>` (Hash160).
- **Capacity**: Pre-allocated for 60,000,000 entries to prevent rehashing resizing.
- **Bloom Filters**: Automatically switches to Bloom Filters if RAM is constrained (False Positive Rate: $1 \times 10^{-8}$).
- **Thread Safety**: The database is loaded into an `Arc` (Atomic Reference Counted) pointer, allowing read-only access across all threads without locking contention.

### 3. Concurrency
- **Rayon**: Uses `rayon` for work-stealing parallelism.
- **Batching**: Keys are processed in batches (default: 2048) to keep CPU cache hot (L1/L2) and minimize context switching overhead.

---

## 📥 Installation

### Prerequisites
- **OS**: Linux (Server/Desktop) or macOS (Apple Silicon Supported).
- **Space**: ~2.5GB Free Disk Space (for UTXO database).

### Quick Start (The "One Command" Setup)
We have built a smart launcher that handles everything.

```bash
git clone https://github.com/Arunkumarkethana/btc_hunter.git
cd btc_hunter
chmod +x hunter
./hunter
```

**The script will:**
1. Install Rust (`cargo`), Git, and system dependencies automatically.
2. Detect if the **UTXO Database** is missing and prompt to download it (2.2GB).
3. Compile the project in `release` mode (max optimization).
4. Launch the interface.

---

## 🖥 Usage

### Dashboard
Once running, you will see the live dashboard:

```text
[Speed: 1.82 M/s] [API: 0/s] [Total: 15.20 B] [Found: 💰0 📜0 💾0]
```

- **Speed**: Keys generated and checked per second.
- **Total**: Total keys checked since start (B = Billions).
- **Found**:
  - 💰 **Funds**: Private key with >0 BTC balance.
  - 📜 **History**: Private key that *used to have* BTC (Empty now).
  - 💾 **Offline**: Match found in local DB (Needs verification).

### Telegram Alerts
To receive instant notifications on your phone:
1. Create a bot with `@BotFather`.
2. Edit `telegram.json`:
```json
{
    "token": "YOUR_BOT_TOKEN",
    "chat_id": "YOUR_CHAT_ID"
}
```
3. Restart hunter. You will get a "🚀 STARTED" message.

---

## 📂 File Structure
- `src/main.rs`: Core logic, threading, UI, and API swarm.
- `src/checker.rs`: High-performance database engine (HashSet/Bloom).
- `src/generator.rs`: CSPRNG key generation.
- `hunter`: Smart BASH launcher script.
- `utxo_manager.py`: Python tool for downloading/syncing the database.

---

## 🔒 Security
- **No Private Keys Saved**: Keys are generated in RAM, checked, and immediately overwritten. They are NEVER written to disk unless they have a balance.
- **Credentials**: `telegram.json` and `utxo_data/` are hard-coded into `.gitignore` to prevent accidental leaks.
- **Open Source**: Verify the code yourself. No hidden "dev fees" or "backdoors".

---

## 🤝 Contributing
1. Fork the repo.
2. Create a feature branch.
3. Submit a Pull Request.

**Happy Hunting!** 🏹
