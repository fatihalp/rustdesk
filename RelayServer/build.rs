fn main() {
    protobuf_codegen::Codegen::new()
        .pure()
        .cargo_out_dir("protos")
        .inputs(["proto/rendezvous.proto", "proto/id_pk.proto"])
        .include("proto")
        .customize(protobuf_codegen::Customize::default().tokio_bytes(true))
        .run_from_script();
}
