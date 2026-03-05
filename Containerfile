FROM debian:testing-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --chmod=755 ties /app/
ENTRYPOINT ["/app/ties", "start"]
