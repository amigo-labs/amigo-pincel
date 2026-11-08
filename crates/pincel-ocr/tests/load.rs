//! Loading models (docs/specs/ocr-recognizer.md AC-01, AC-02).

use pincel_ocr::{LoadError, Model};

const ENG: &[u8] = include_bytes!("../models/eng.traineddata");
const DEU: &[u8] = include_bytes!("../models/deu.traineddata");

/// Component indices of the `.traineddata` container (`TessdataType`).
const LSTM: usize = 17;
const LSTM_UNICHARSET: usize = 21;
const LSTM_RECODER: usize = 22;

fn offsets(data: &[u8]) -> Vec<i64> {
    let n = u32::from_le_bytes(data[0..4].try_into().expect("4 bytes")) as usize;
    (0..n)
        .map(|i| i64::from_le_bytes(data[4 + 8 * i..12 + 8 * i].try_into().expect("8 bytes")))
        .collect()
}

fn set_offset(data: &mut [u8], index: usize, value: i64) {
    data[4 + 8 * index..12 + 8 * index].copy_from_slice(&value.to_le_bytes());
}

/// The byte range of a component.
fn component(data: &[u8], index: usize) -> std::ops::Range<usize> {
    let offs = offsets(data);
    let start = offs[index] as usize;
    let end = offs[index + 1..]
        .iter()
        .find(|&&o| o != -1)
        .map_or(data.len(), |&o| o as usize);
    start..end
}

#[test]
fn from_bytes_eng_reports_charset_size_and_input_height() {
    // `combine_tessdata -d eng.traineddata` and the first line of its
    // lstm-unicharset: 112 entries; the network spec starts [1,36,0,1.
    let model = Model::from_bytes(ENG).expect("eng loads");
    assert_eq!(model.character_set_size(), 112);
    assert_eq!(model.input_height(), 36);
    assert!(model.version().contains(":eng:"));
}

#[test]
fn from_bytes_deu_reports_charset_size_and_input_height() {
    // deu: 116 unicharset entries, version
    // 4.00.00alpha:deu:synth20170629:[1,36,0,1Ct3,3,16Mp3,3Lfys64Lfx96Lrx96Lfx192O1c1].
    let model = Model::from_bytes(DEU).expect("deu loads");
    assert_eq!(model.character_set_size(), 116);
    assert_eq!(model.input_height(), 36);
}

#[test]
fn from_bytes_text_returns_not_traineddata() {
    assert_eq!(
        Model::from_bytes(b"this is not a model").err(),
        Some(LoadError::NotTraineddata)
    );
    assert_eq!(
        Model::from_bytes(b"").err(),
        Some(LoadError::NotTraineddata)
    );
}

#[test]
fn from_bytes_cut_short_returns_truncated() {
    assert_eq!(
        Model::from_bytes(&ENG[..ENG.len() / 2]).err(),
        Some(LoadError::Truncated)
    );
    // Inside the offset table.
    assert_eq!(
        Model::from_bytes(&ENG[..40]).err(),
        Some(LoadError::Truncated)
    );
}

#[test]
fn from_bytes_huge_size_field_returns_truncated() {
    // The LSTM component starts with the network's type byte and flags; the
    // layer name is a u32 length followed by bytes. Point an offset past the
    // end, and separately claim a 4 GB layer name.
    let mut data = ENG.to_vec();
    set_offset(&mut data, LSTM, i64::MAX / 2);
    assert_eq!(Model::from_bytes(&data).err(), Some(LoadError::Truncated));

    let mut data = ENG.to_vec();
    let lstm = component(&data, LSTM);
    // The first layer is stored by type name: type 0 (1 byte) and the
    // name "Series" (4 + 6), then training (1), backprop (1), flags (4),
    // ni (4), no (4), num_weights (4) and the layer name's length.
    let name_len = lstm.start + 1 + 4 + 6 + 1 + 1 + 4 + 4 + 4 + 4;
    data[name_len..name_len + 4].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
    assert_eq!(Model::from_bytes(&data).err(), Some(LoadError::Truncated));
}

#[test]
fn from_bytes_without_lstm_returns_no_lstm() {
    let mut data = ENG.to_vec();
    set_offset(&mut data, LSTM, -1);
    assert_eq!(Model::from_bytes(&data).err(), Some(LoadError::NoLstm));
}

#[test]
fn from_bytes_without_charsets_returns_charset_unreadable() {
    let mut data = ENG.to_vec();
    set_offset(&mut data, LSTM_RECODER, -1);
    assert_eq!(
        Model::from_bytes(&data).err(),
        Some(LoadError::CharsetUnreadable)
    );
    let mut data = ENG.to_vec();
    let unicharset = component(&data, LSTM_UNICHARSET);
    data[unicharset.start..unicharset.start + 3].copy_from_slice(b"abc");
    assert_eq!(
        Model::from_bytes(&data).err(),
        Some(LoadError::CharsetUnreadable)
    );
}

/// Builds a `.traineddata` archive from 24 optional components.
fn archive(parts: &[Option<&[u8]>]) -> Vec<u8> {
    let header = 4 + 8 * parts.len();
    let mut out = (parts.len() as u32).to_le_bytes().to_vec();
    let mut body = Vec::new();
    for p in parts {
        match p {
            Some(bytes) => {
                out.extend_from_slice(&((header + body.len()) as i64).to_le_bytes());
                body.extend_from_slice(bytes);
            }
            None => out.extend_from_slice(&(-1i64).to_le_bytes()),
        }
    }
    out.extend_from_slice(&body);
    out
}

fn parts(data: &[u8]) -> Vec<Option<&[u8]>> {
    let offs = offsets(data);
    (0..offs.len())
        .map(|i| (offs[i] >= 0).then(|| &data[component(data, i)]))
        .collect()
}

#[test]
fn from_bytes_rebuilt_archive_loads() {
    assert!(Model::from_bytes(&archive(&parts(ENG))).is_ok());
}

#[test]
fn from_bytes_charsets_of_other_model_returns_charset_mismatch() {
    // eng's network with deu's unicharset and recoder: deu codes more
    // characters than eng's softmax has outputs.
    let mut p = parts(ENG);
    let d = parts(DEU);
    p[LSTM_UNICHARSET] = d[LSTM_UNICHARSET];
    p[LSTM_RECODER] = d[LSTM_RECODER];
    assert_eq!(
        Model::from_bytes(&archive(&p)).err(),
        Some(LoadError::CharsetMismatch)
    );
}

#[test]
fn from_bytes_unknown_layer_returns_unsupported_layer() {
    // The first layer is serialized by its type name, "Series"; rename it
    // to a type Tesseract does not have.
    let mut data = ENG.to_vec();
    let lstm = component(&data, LSTM);
    assert_eq!(&data[lstm.start + 5..lstm.start + 11], b"Series");
    data[lstm.start + 5..lstm.start + 11].copy_from_slice(b"Bogus!");
    assert!(matches!(
        Model::from_bytes(&data).err(),
        Some(LoadError::UnsupportedLayer(_))
    ));
}

#[test]
fn from_bytes_float_weights_returns_float_weights() {
    // Clear TF_INT_MODE in the recognizer's training flags, as a
    // tessdata_best model has it: they follow the network and its spec.
    let mut data = ENG.to_vec();
    let lstm = component(&data, LSTM);
    let flags = find_training_flags(&data[lstm.clone()]) + lstm.start;
    data[flags] &= !1;
    assert_eq!(
        Model::from_bytes(&data).err(),
        Some(LoadError::FloatWeights)
    );
}

/// Offset of the training flags inside the LSTM component: right after the
/// network spec string, which starts with "[1,36,0,1".
fn find_training_flags(lstm: &[u8]) -> usize {
    let spec = lstm
        .windows(9)
        .position(|w| w == b"[1,36,0,1")
        .expect("network spec in the LSTM component");
    let len = u32::from_le_bytes(lstm[spec - 4..spec].try_into().expect("4 bytes")) as usize;
    spec + len
}
