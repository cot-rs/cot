FROM docker.io/library/rust:1.93 AS builder
WORKDIR /usr/src/cot
COPY . .
WORKDIR /usr/src/cot/docs-site
RUN cargo install --path . --locked

FROM docker.io/library/debian:13-slim
COPY --from=builder /usr/local/cargo/bin/docs-site /usr/local/bin/docs-site
RUN mkdir /app

RUN apt-get update &&  \
    apt-get install -y --no-install-recommends tini=0.19.* && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["docs-site", "-l", "0.0.0.0:8000"]
EXPOSE 8000
