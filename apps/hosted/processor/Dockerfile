FROM docker.io/library/alpine:3.22.1@sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1

COPY foundation-probe /app/foundation-probe
RUN chmod 0555 /app/foundation-probe

USER 65534:65534
CMD ["/bin/sleep", "infinity"]
