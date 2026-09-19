use std::collections::BTreeMap;

use futures_util::{StreamExt, stream};
use yeravich_core::{
    CapabilityError, LanguagePair, ProviderProfile, RequestId, TranslationEvent,
    TranslationProvider, TranslationRequest, TranslationStream,
};

struct FakeProvider;

impl TranslationProvider for FakeProvider {
    fn translate(&self, request: TranslationRequest) -> Result<TranslationStream, CapabilityError> {
        Ok(Box::pin(stream::iter([
            Ok(TranslationEvent::Delta {
                id: request.id,
                text: "你".into(),
            }),
            Ok(TranslationEvent::Completed {
                id: request.id,
                text: "你好".into(),
            }),
        ])))
    }
}

#[test]
fn fake_provider_emits_delta_then_completed_without_network() {
    futures_executor::block_on(async {
        let stream = FakeProvider
            .translate(TranslationRequest {
                id: RequestId(7),
                text: "hello".into(),
                languages: LanguagePair::default(),
                provider: ProviderProfile {
                    name: "fake".into(),
                    adapter_id: "fake".into(),
                    public: BTreeMap::default(),
                    secret_ref: None,
                },
            })
            .unwrap();
        let events: Vec<_> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                TranslationEvent::Delta { .. },
                TranslationEvent::Completed { .. }
            ]
        ));
    });
}
