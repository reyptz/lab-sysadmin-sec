# syntax=docker/dockerfile:1
# Moteur Shinra (FastAPI) — image immuable déployée par socle sur RHEL
# (service `shinra-engine`, playbook deploy_service.yml, Podman/Quadlet).
#
# Données persistantes (SQLite, ChromaDB, cache des modèles) : /app/data,
# monté depuis l'hôte (/var/lib/shinra). Le moteur y écrit par chemins relatifs.

FROM python:3.11-slim AS build
ENV PIP_NO_CACHE_DIR=1 PIP_DISABLE_PIP_VERSION_CHECK=1
WORKDIR /build
COPY requirements.txt .
# PyTorch CPU (dépendance de sentence-transformers) : image bien plus légère
# que la variante CUDA, suffisante pour l'inférence d'embeddings.
RUN python -m venv /opt/venv \
    && /opt/venv/bin/pip install --extra-index-url https://download.pytorch.org/whl/cpu -r requirements.txt

FROM python:3.11-slim
ENV PATH=/opt/venv/bin:$PATH \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    SHINRA_HOST=0.0.0.0 \
    HF_HOME=/app/data/.cache/huggingface
RUN useradd --system --uid 10001 --home-dir /app/data --no-create-home shinra \
    && install -d -o 10001 -g 10001 -m 0750 /app/data
COPY --from=build /opt/venv /opt/venv
COPY --chown=root:root *.py /app/

USER 10001:10001
WORKDIR /app/data
VOLUME ["/app/data"]
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD ["python", "-c", "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8000/health', timeout=4).status == 200 else 1)"]
ENTRYPOINT ["python", "/app/main.py"]
