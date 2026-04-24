//! The compiled-in list of models Hey Dot can download, each file pinned to an upstream
//! commit with its sha256 and size.
//! Must not fetch anything or decide which model a machine should use.

#[derive(Debug, PartialEq, Eq)]
pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Model {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Smallest RAM, in GiB, the model is meant to run on. 0 for models too small to matter.
    pub min_ram_gb: u64,
    pub files: &'static [ModelFile],
}

pub static QWEN3_VL_2B: Model = Model {
    id: "qwen3-vl-2b-instruct-q4km",
    display_name: "Qwen3-VL 2B",
    min_ram_gb: 8,
    files: &[
        ModelFile {
            name: "Qwen3VL-2B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/52d6c8ffea26cc873ac5ad116f8631268d7eb503/Qwen3VL-2B-Instruct-Q4_K_M.gguf",
            sha256: "089d75c52f4b7ffc56ba998ffc50aae89fcafc755f9e7208aacca281dca6c2ae",
            bytes: 1_107_409_952,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-2B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/52d6c8ffea26cc873ac5ad116f8631268d7eb503/mmproj-Qwen3VL-2B-Instruct-F16.gguf",
            sha256: "c3d5afbef5287953acd57b4043d2269456e5761a4eaccb3b71b062996970aea5",
            bytes: 819_394_848,
        },
    ],
};

pub static QWEN3_VL_4B: Model = Model {
    id: "qwen3-vl-4b-instruct-q4km",
    display_name: "Qwen3-VL 4B",
    min_ram_gb: 16,
    files: &[
        ModelFile {
            name: "Qwen3VL-4B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266/Qwen3VL-4B-Instruct-Q4_K_M.gguf",
            sha256: "66358cb18bb6b3b1b6675aa412c7a88ef01d228f481184d13668e5201c730a0a",
            bytes: 2_497_281_664,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-4B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266/mmproj-Qwen3VL-4B-Instruct-F16.gguf",
            sha256: "256f3a43bd4205ffef48d6b92715e1e70b5b0e9aef06522584967513a9985331",
            bytes: 836_180_256,
        },
    ],
};

pub static QWEN3_VL_8B: Model = Model {
    id: "qwen3-vl-8b-instruct-q4km",
    display_name: "Qwen3-VL 8B",
    min_ram_gb: 32,
    files: &[
        ModelFile {
            name: "Qwen3VL-8B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/f982a07559d4a2f6c8744d840bf6fccab30eea96/Qwen3VL-8B-Instruct-Q4_K_M.gguf",
            sha256: "67d1659bfe71b89d50b45a4ad1a9e5b997e5bb16ce5da66a6a6167abd569e9e2",
            bytes: 5_027_784_800,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-8B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/f982a07559d4a2f6c8744d840bf6fccab30eea96/mmproj-Qwen3VL-8B-Instruct-F16.gguf",
            sha256: "ca524100ebf825c9a870db1c580d03879e0da0ab2541697e2458e64891cf9d38",
            bytes: 1_159_029_824,
        },
    ],
};

pub static PARAKEET_TDT_V3: Model = Model {
    id: "parakeet-tdt-0.6b-v3-int8",
    display_name: "Parakeet TDT 0.6B v3",
    min_ram_gb: 0,
    files: &[
        ModelFile {
            name: "encoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/encoder.int8.onnx",
            sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
            bytes: 652_184_281,
        },
        ModelFile {
            name: "decoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/decoder.int8.onnx",
            sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
            bytes: 11_845_275,
        },
        ModelFile {
            name: "joiner.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/joiner.int8.onnx",
            sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
            bytes: 6_355_277,
        },
        ModelFile {
            name: "tokens.txt",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/tokens.txt",
            sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
            bytes: 93_939,
        },
    ],
};

pub static SILERO_VAD_V6: Model = Model {
    id: "silero-vad-v6",
    display_name: "Silero VAD v6",
    min_ram_gb: 0,
    files: &[ModelFile {
        name: "silero_vad.onnx",
        url: "https://raw.githubusercontent.com/snakers4/silero-vad/fba061dc5559f696e62171e9a0741782b0fdc23c/src/silero_vad/data/silero_vad.onnx",
        sha256: "597d30b3ec076608d059477bb14cfeffdf951bf5cae370d38f65d33bbfe82004",
        bytes: 2_327_524,
    }],
};

pub static CATALOG: [&Model; 5] = [
    &QWEN3_VL_2B,
    &QWEN3_VL_4B,
    &QWEN3_VL_8B,
    &PARAKEET_TDT_V3,
    &SILERO_VAD_V6,
];
