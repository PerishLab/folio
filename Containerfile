FROM docker.io/library/debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 10001 folio \
    && useradd --uid 10001 --gid 10001 --home /folio --shell /usr/sbin/nologin folio
WORKDIR /folio
COPY folio-api /usr/local/bin/folio-api
RUN chmod 0755 /usr/local/bin/folio-api && chown folio:folio /folio
USER folio
EXPOSE 8080
ENTRYPOINT ["folio-api"]
CMD ["serve", "/folio"]
