# syntax=docker/dockerfile:1
# Image du serveur socle : binaire Rust + outils pilotés (git, kubectl,
# ansible-core, terraform, restic). Les outils proviennent de dépôts APT
# signés (vérification GPG par apt) ; ajoutez velero, aws, az, gcloud dans
# une image dérivée si vos politiques les utilisent.
#
#   docker build -t socle:dev socle/
#   docker run --rm -p 8080:8080 -v "$PWD:/repo" socle:dev serve

ARG DEBIAN_RELEASE=bookworm

FROM rust:1-${DEBIAN_RELEASE} AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p socle \
    && install -m 0755 target/release/socle /usr/local/bin/socle

FROM debian:${DEBIAN_RELEASE}-slim
ARG KUBERNETES_MINOR=v1.31
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl gnupg git openssh-client ansible-core python3-boto3 restic tini; \
    install -d -m 0755 /etc/apt/keyrings; \
    curl -fsSL "https://pkgs.k8s.io/core:/stable:/${KUBERNETES_MINOR}/deb/Release.key" | gpg --dearmor -o /etc/apt/keyrings/kubernetes.gpg; \
    echo "deb [signed-by=/etc/apt/keyrings/kubernetes.gpg] https://pkgs.k8s.io/core:/stable:/${KUBERNETES_MINOR}/deb/ /" > /etc/apt/sources.list.d/kubernetes.list; \
    curl -fsSL https://apt.releases.hashicorp.com/gpg | gpg --dearmor -o /etc/apt/keyrings/hashicorp.gpg; \
    . /etc/os-release; \
    echo "deb [signed-by=/etc/apt/keyrings/hashicorp.gpg] https://apt.releases.hashicorp.com ${VERSION_CODENAME} main" > /etc/apt/sources.list.d/hashicorp.list; \
    apt-get update; \
    apt-get install -y --no-install-recommends kubectl terraform; \
    apt-get purge -y gnupg; apt-get autoremove -y; rm -rf /var/lib/apt/lists/*; \
    useradd --system --uid 10001 --home-dir /var/lib/socle --create-home socle; \
    # Collections Ansible des playbooks pilotés (Podman, AWS RDS…).
    ansible-galaxy collection install --collections-path /usr/share/ansible/collections \
      community.general ansible.posix containers.podman amazon.aws

COPY --from=build /usr/local/bin/socle /usr/local/bin/socle

USER 10001:10001
WORKDIR /repo
ENV SOCLE_BIND=0.0.0.0:8080 \
    SOCLE_SPEC=/repo/socle.yaml \
    SOCLE_REPO_ROOT=/repo \
    SOCLE_LOG_FORMAT=json
EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/socle"]
CMD ["serve"]
