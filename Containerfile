FROM debian:testing-slim
COPY --chmod=755 ties /app/
ENTRYPOINT ["/app/ties", "start"]
