FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p sql-dialect-fmt --bin sql-dialect-fmt

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/sql-dialect-fmt /usr/local/bin/sql-dialect-fmt
ENTRYPOINT ["/usr/local/bin/sql-dialect-fmt"]
CMD ["--help"]
