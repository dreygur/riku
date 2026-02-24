FROM public.ecr.aws/ubuntu/ubuntu:24.04_stable

RUN apt-get update -y && apt-get install -y openssh-server git
RUN mkdir /var/run/sshd
RUN echo 'root:password' | chpasswd

# git defaultBranch main
RUN git config --global init.defaultBranch

# Configure SSH to use id_ed25519 key for authentication
RUN mkdir -p /root/.ssh && chmod 700 /root/.ssh
COPY tests/e2e/id_ed25519.pub /root/.ssh/authorized_keys
RUN chmod 600 /root/.ssh/authorized_keys

# Configure sshd to disable password auth and use key-based auth
RUN sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config && \
    sed -i 's/PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config

# Copy statically linked riku binary
COPY target/x86_64-unknown-linux-musl/release/riku /root/riku
RUN chmod +x /root/riku

# Initialize riku and add to PATH
RUN /root/riku init --no-systemd || true

COPY tests/e2e/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 22

CMD ["/entrypoint.sh"]
