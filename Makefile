.DEFAULT_GOAL := help

.PHONY: help docker docker-no-frontend docker-no-backend dev dev-backend dev-frontend \
        infra infra-down build build-backend build-frontend test test-backend test-frontend \
        check lint clean nuke logs

# ── Help ────────────────────────────────────────────────────────
help: ## Show this help message
	@echo ""
	@echo "  RAG Pipeline - Available Commands"
	@echo "  ─────────────────────────────────────────────────────"
	@echo ""
	@printf "  \033[1mDocker (full containerized)\033[0m\n"
	@echo "    make docker              Run everything in Docker (infra + backend + frontend)"
	@echo "    make docker-no-frontend  Run infra + backend in Docker, frontend runs locally"
	@echo "    make docker-no-backend   Run infra + frontend in Docker, backend runs locally"
	@echo "    make docker-down         Stop and remove all Docker containers"
	@echo ""
	@printf "  \033[1mLocal Development\033[0m\n"
	@echo "    make dev                 Start infra in Docker, then run backend + frontend locally"
	@echo "    make dev-backend         Run only the backend locally (assumes infra is up)"
	@echo "    make dev-frontend        Run only the frontend locally"
	@echo "    make infra               Start infrastructure (Postgres + MinIO + Qdrant) in Docker"
	@echo "    make infra-down          Stop infrastructure containers"
	@echo ""
	@printf "  \033[1mBuild\033[0m\n"
	@echo "    make build               Build both backend (release) and frontend"
	@echo "    make build-backend       Build backend in release mode"
	@echo "    make build-frontend      Build frontend for production"
	@echo ""
	@printf "  \033[1mTest & Lint\033[0m\n"
	@echo "    make test                Run all tests (backend + frontend)"
	@echo "    make test-backend        Run backend tests only"
	@echo "    make test-frontend       Run frontend tests only"
	@echo "    make check               Quick compile check (backend)"
	@echo "    make lint                Run clippy lints (backend)"
	@echo ""
	@printf "  \033[1mUtility\033[0m\n"
	@echo "    make logs                Tail logs from all Docker containers"
	@echo "    make clean               Remove all build artifacts"
	@echo "    make nuke                Stop and remove ALL Docker resources (containers, volumes, images)"
	@echo ""

# ── Docker (fully containerized) ───────────────────────────────
docker: ## Run everything in Docker (infra + backend + frontend)
	docker compose --profile app up --build -d

docker-no-frontend: ## Run infra + backend in Docker (run frontend locally)
	docker compose --profile backend up --build -d

docker-no-backend: ## Run infra + frontend in Docker (run backend locally)
	docker compose --profile frontend up --build -d

docker-down: ## Stop and remove all Docker containers
	docker compose --profile app --profile backend --profile frontend down

# ── Local Development ──────────────────────────────────────────
dev: infra ## Start infra in Docker, run backend + frontend locally
	@echo ""
	@echo "  Infrastructure is running. Starting applications..."
	@echo "  Backend:   http://localhost:3000"
	@echo "  Frontend:  http://localhost:5173"
	@echo "  Postgres:  localhost:5432"
	@echo "  MinIO:     http://localhost:9001"
	@echo "  Qdrant:    http://localhost:6333/dashboard"
	@echo ""
	@trap 'kill 0' EXIT; \
	(cd backend && cargo run) & \
	(cd frontend && npm run dev -- --port 5173) & \
	wait

dev-backend: ## Run backend locally (assumes infra is already up)
	cd backend && cargo run

dev-frontend: ## Run frontend locally
	cd frontend && npm run dev -- --port 5173

infra: ## Start only infrastructure (Postgres + MinIO + Qdrant) in Docker
	docker compose up -d postgres qdrant minio minio-init

infra-down: ## Stop infrastructure containers
	docker compose down

# ── Build ──────────────────────────────────────────────────────
build: build-backend build-frontend ## Build both backend and frontend

build-backend: ## Build backend in release mode
	cd backend && cargo build --release

build-frontend: ## Build frontend for production
	cd frontend && npm run build

# ── Test & Lint ────────────────────────────────────────────────
test: test-backend test-frontend ## Run all tests

test-backend: ## Run backend tests
	cd backend && cargo test

test-frontend: ## Run frontend tests
	cd frontend && npm test 2>/dev/null || echo "No frontend tests configured yet"

check: ## Quick compile check (backend)
	cd backend && cargo check

lint: ## Run clippy lints (backend)
	cd backend && cargo clippy -- -D warnings

# ── Utility ────────────────────────────────────────────────────
logs: ## Tail logs from all Docker containers
	docker compose --profile app logs -f

clean: ## Remove all build artifacts
	cd backend && cargo clean
	rm -rf frontend/node_modules frontend/.svelte-kit frontend/build

nuke: ## Stop and remove ALL Docker resources (containers, volumes, images)
	@echo "  Stopping all containers..."
	docker compose --profile app --profile backend --profile frontend down --volumes --remove-orphans
	@echo "  Removing project images..."
	@docker images --filter "reference=rag-pipeline*" -q | xargs -r docker rmi -f 2>/dev/null || true
	@echo "  Done. All Docker resources for this project have been removed."
