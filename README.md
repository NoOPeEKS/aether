# Aether 🚀

[![License: Apache License 2.0](https://img.shields.io/badge/license-Apache%20License%202.0-blue)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.91%2B-orange)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/aether-cli)](https://crates.io/crates/aether-cli)

A blazingly fast distributed task executor for Python scripts. Aether provides a robust cluster architecture to run your Python workloads across multiple machines with ease.

## ✨ Features

- ⚡ **High Performance**: Asynchronous Rust core for maximum throughput.
- 🐍 **Python Focused**: Execute Python scripts with proper isolation and resource management.
- 🔄 **Priority Queuing**: Support for high, medium, and low priority task scheduling.
- 🎯 **Smart Matching**: Automatic worker assignment based on GPU availability and CPU architecture.
- 🌐 **HTTP API**: RESTful interface for task submission and monitoring.
- 🔗 **JRPC Protocol**: JSON-RPC communication between brokers and workers.
- 🛡️ **Graceful Shutdown**: Clean task handling during system interruptions.
- 🖥️ **CLI Tools**: Intuitive command-line interface for all operations.

## 📋 Table of Contents

- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Installation](#installation)
- [Usage](#usage)
- [API](#api)
- [Contributing](#contributing)
- [License](#license)

## 🚀 Quick Start

Get Aether running in under 1 minute!

### 1. Start the Broker

```bash
aether broker start --api-port 8080 --jrpc-port 9090
```

### 2. Launch a Worker

In a new terminal:

```bash
# GPU and Architecture capabilities are set to `false` and `x86_64` by default.
aether worker start --worker-id worker1 --broker-ip 127.0.0.1 --broker-port 9090
```

### 3. Submit Your First Task

Create `hello.py`:

```python
import time
print("Hello from Aether! 🚀")
time.sleep(1)
print("Task completed successfully!")
```

Submit it:

```bash
aether task submit --broker-ip 127.0.0.1 --broker-api-port 8080 --task-file hello.py --name "Hello World"
```

### 4. Check Status

```bash
aether task check --broker-ip 127.0.0.1 --broker-api-port 8080 --task-id <task-uuid>
```

Output:
```json
{
  "id": "59d5ca42-93e7-4b11-8927-e25c519db283",
  "name": "Hello World",
  "code_b64": "aW1wb3J0IHRpbWUKcHJpbnQoIkhlbGxvIGZyb20gQWV0aGVyISDwn5qAIikKdGltZS5zbGVlcCgxKQpwcmludCgiVGFzayBjb21wbGV0ZWQgc3VjY2Vzc2Z1bGx5ISIpCg==",
  "result": {
    "exit_code": 0,
    "stderr": "",
    "stdout": "Hello from Aether! 🚀\nTask completed successfully!\n"
  },
  "status": "completed",
  "capabilities": {
    "gpu": false,
    "arch": "x86_64"
  }
}
```

🎉 Congratulations! You've just executed your first distributed Python task.

## 🏗️ Architecture

```
┌─────────────┐    HTTP     ┌─────────────┐
│   Client    │ ──────────► │   Broker    │
└─────────────┘             └─────────────┘
                                 │
                                 │ JRPC
                                 ▼
                         ┌─────────────┐
                         │   Worker    │
                         │  (Python)   │
                         └─────────────┘
```

- **Broker**: Central coordinator managing task queues and worker registry
- **Workers**: Execute Python scripts with resource matching
- **Clients**: Submit tasks via HTTP API and monitor progress

## 📦 Installation

### From Source

Requires Rust 1.88+.

```bash
git clone https://github.com/yourusername/aether.git
cd aether
cargo build --release
```

### From Crates.io (when available)

```bash
cargo install aether-cli
```

## 💻 Usage

### Broker Commands

```bash
# Start broker
aether broker start --api-port 8080 --jrpc-port 9090
```

### Worker Commands

```bash
# Start worker with GPU support
aether worker start --worker-id gpu-worker --broker-ip 10.0.0.1 --broker-port 9090 --gpu true --arch aarch64
```

### Task Management

```bash
# Submit task
aether task submit --broker-ip 127.0.0.1 --broker-api-port 8080 --task-file script.py --name "Data Analysis"

# Check status
aether task check --broker-ip 127.0.0.1 --broker-api-port 8080 --task-id 550e8400-e29b-41d4-a716-446655440000

# List all tasks (future feature)
aether task list --broker-ip 127.0.0.1 --broker-api-port 8080
```

## 🔌 API

### Endpoints

- `POST /api/v1/tasks` - Submit a new task
- `GET /api/v1/tasks/{id}` - Get task status and result
- `GET /api/v1/tasks` - List all tasks
- `GET /api/v1/health` - Health check

## 🤝 Contributing

- 🐛 Found a bug? [Open an issue](https://github.com/NoOPeEKS/aether/issues)
- 💡 Have a feature request? [Start a discussion](https://github.com/NoOPeEKS/aether/discussions)
- 🔧 Want to contribute code? Check out our [development docs](docs/DEV.md)

## 📄 License

Licensed under the Apache 2.0 License. See [LICENSE](LICENSE) for details.

---

Built with ❤️ using Rust.
