# Docker Development Environment for smelt

This Docker setup provides a complete, sandboxed development environment for the smelt project with all dependencies pre-installed.

## Why Use Docker?

- **Isolated Environment**: Run Claude Code with more permissive settings without affecting your host system
- **Consistent Dependencies**: All team members use the same Rust, Spark, and Node.js versions
- **No System Pollution**: Keep your host system clean - all build artifacts stay in Docker volumes
- **Safe Git Operations**: Configured for local commits only - no accidental pushes to remote
- **Easy Spark Testing**: Apache Spark pre-installed for backend testing

## Prerequisites

- Docker (20.10+)
- Docker Compose (2.0+)

## Quick Start

### 1. Build the Docker Image

```bash
docker-compose build
```

This will install:
- Ubuntu 22.04 base
- Rust toolchain (latest stable)
- Apache Spark 3.5.0 with Hadoop 3
- Node.js 20 (LTS)
- **Claude Code CLI**
- Git configured for local-only commits

### 2. Run Claude Code Inside the Container

**Easiest method** - Use the convenience script:

```bash
./claude.sh
```

Or manually:

```bash
# Build and start the container
docker-compose up -d

# Run Claude Code CLI inside the container
docker-compose exec smelt-dev claude
```

That's it! Claude will run inside the sandboxed container where:
- All development tools are pre-installed
- Git push is disabled (commits stay local)
- Changes sync to your host filesystem
- You can review commits on the host before manually pushing

### Alternative: Manual Development Inside Container

If you want to work inside the container manually (without Claude):

```bash
# Enter the container
docker-compose exec smelt-dev bash

# Inside the container, all commands work normally:
cargo build
cargo test
cargo clippy --all-targets
cargo run --example example1_naive

# Build VSCode extension
cd editors/vscode
npm install
npm run compile
```

### 3. Stop the Container

```bash
docker-compose down
```

## How It Works

### Volume Mounts

The docker-compose.yml mounts your source code into the container:

- Host: `/Users/andrewbrowne/code/smelt`
- Container: `/workspace`

Changes you make in the container are immediately reflected on the host and vice versa.

### Build Artifact Caching

Build artifacts are stored in Docker volumes for performance:
- `target/` - Rust build artifacts
- `editors/vscode/node_modules/` - Node.js dependencies

This prevents conflicts between host and container builds and improves performance.

### Git Configuration

The container uses a restricted git configuration (`docker/gitconfig`) that:
- Sets identity to "Claude Code (Docker)" <claude@docker.local>
- **Disables push operations** to prevent accidental remote changes
- Allows normal commits, which stay in the local repository

**Important**: Commits made in the container are visible on the host. You can:
1. Review commits on your host system
2. Push them manually from the host if desired
3. Or discard them if they're not needed

## Running Claude Code Inside the Container

This Docker setup is specifically designed for running the Claude Code CLI *inside* the container, giving you a more permissive environment.

### Starting Claude Code in the Container

**Easiest method** - Use the convenience script:

```bash
./claude.sh
```

Or manually:

```bash
# On your host machine: Start the container
docker-compose up -d

# Run Claude Code inside the container
docker-compose exec smelt-dev claude
```

### Why Run Claude Code in Docker?

When running Claude Code inside the container, you get:
- **Isolated Environment**: Claude can be more permissive without affecting your host system
- **Push Protection**: Git push is disabled - commits stay local
- **No Credential Access**: Container can't access your SSH keys or GitHub credentials
- **Sandboxed Execution**: All operations are contained within the Docker environment

### What Claude Can Do

Inside the container, Claude Code can safely:
- ✅ Make local commits
- ✅ Run all build and test commands (cargo, npm, spark-submit)
- ✅ Modify source files (changes sync to host via mounted volume)
- ✅ Install dependencies with Cargo/npm
- ❌ **Cannot push to GitHub** (explicitly disabled)
- ❌ **Cannot access host SSH keys**

### Workflow

1. **Launch Claude**: `./claude.sh` (or manually: `docker-compose up -d && docker-compose exec smelt-dev claude`)
2. **Claude works on tasks**: Makes changes, runs tests, commits locally
3. **Review on host**: Exit Claude, review changes with `git log`, `git diff`
4. **Push manually** (if desired): From host, push approved commits

This gives you the benefit of Claude's autonomous capabilities while maintaining safety through isolation.

## Advanced Usage

### Rebuilding After Dockerfile Changes

```bash
docker-compose build --no-cache
```

### Viewing Container Logs

```bash
docker-compose logs -f smelt-dev
```

### Cleaning Up Volumes

To free up space by removing build artifact caches:

```bash
docker-compose down -v
```

**Warning**: This will delete cached Rust and Node.js builds, requiring a full rebuild next time.

### Customizing the Environment

Edit `Dockerfile` to:
- Change Rust version (modify rustup install command)
- Change Spark version (modify SPARK_VERSION environment variable)
- Add additional tools

Edit `docker-compose.yml` to:
- Change volume mounts
- Add environment variables
- Configure networking

## Troubleshooting

### Permission Issues

If you encounter permission issues with files created by Docker:

```bash
# Fix ownership on host
sudo chown -R $(whoami):$(whoami) target/
```

### Port Conflicts

If the LSP server port conflicts, modify `docker-compose.yml`:

```yaml
ports:
  - "9257:9257"  # Change the first number
```

### Out of Disk Space

Docker images and volumes can consume significant space:

```bash
# See disk usage
docker system df

# Clean up unused images/volumes
docker system prune -a
```

## Files Created

- `Dockerfile` - Container image definition
- `docker-compose.yml` - Container orchestration config
- `docker/gitconfig` - Restricted git configuration
- `.dockerignore` - Files excluded from Docker build context
- `claude.sh` - Convenience script to launch Claude Code in container
- `DOCKER.md` - This file

## Security Notes

- The container does NOT have access to your host's SSH keys
- Git push operations are explicitly disabled
- The container cannot modify files outside the mounted workspace
- Network access is unrestricted (for package downloads) - you can enable network isolation in docker-compose.yml if needed

## Comparison: Host vs Docker

| Feature | Host Development | Docker Development |
|---------|-----------------|-------------------|
| Setup Time | Manual dependency installation | One `docker-compose build` |
| Isolation | Affects host system | Fully isolated |
| Git Push | Enabled | Disabled (safety) |
| Build Speed | Slightly faster (native) | Very close (cached volumes) |
| Disk Usage | Build artifacts in workspace | Build artifacts in volumes |
| Spark Testing | Manual Spark install required | Pre-installed |
| Claude Code Permissions | Need to be careful | Can be permissive |

## Next Steps

1. Build the image: `docker-compose build` (already done!)
2. Launch Claude Code: `./claude.sh`
3. Claude makes changes and commits locally
4. Review commits on host before pushing manually

For more information, see the main CLAUDE.md file.
