# Third-party notices

Tertius depends on open-source Rust crates and npm packages listed in `Cargo.lock` and `apps/desktop/package-lock.json`. Those projects retain their own copyright and license terms.

## Bundled fonts

The desktop interface includes these font files:

| Font | Copyright | License |
|---|---|---|
| Archivo | Copyright 2020 The Archivo Project Authors | SIL Open Font License 1.1 |
| JetBrains Mono | Copyright 2020 The JetBrains Mono Project Authors | SIL Open Font License 1.1 |
| Spectral | Copyright 2017 The Spectral Project Authors | SIL Open Font License 1.1 |

Project sources:

- Archivo: <https://github.com/Omnibus-Type/Archivo>
- JetBrains Mono: <https://github.com/JetBrains/JetBrainsMono>
- Spectral: <https://github.com/productiontype/Spectral>

The common license text is included at [licenses/OFL-1.1.txt](licenses/OFL-1.1.txt).

## Speech model

The speech-model archive lives outside the source repository. Tertius downloads a Parakeet TDT 0.6B v3 ONNX conversion during first-time setup.

- Original model: NVIDIA Parakeet TDT 0.6B v3
- Model card: <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>
- License: Creative Commons Attribution 4.0 International
- ONNX model source referenced by `transcribe-rs`: <https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx>
- CC BY 4.0 text: <https://creativecommons.org/licenses/by/4.0/legalcode>

Users who download or redistribute model files are responsible for following the model's license and attribution requirements.
