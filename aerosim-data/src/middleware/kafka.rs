use std::{
    error::Error,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use futures_util::StreamExt;
use rdkafka::{
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
    consumer::{Consumer, StreamConsumer},
    message::Message,
    producer::{FutureProducer, FutureRecord},
    ClientConfig, TopicPartitionList,
};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::task;

use crate::middleware::{
    CallbackClosureRaw, Middleware, MiddlewareRaw, Serializer, SerializerEnum,
};

#[cfg(feature = "python")]
use {
    crate::middleware::{Metadata, PyMiddleware, PySerializer},
    crate::types::TimeStamp,
    pyo3::prelude::*,
};

#[cfg_attr(feature = "python", pyclass)]
pub struct KafkaSerializer;

impl Serializer for KafkaSerializer {
    fn serializer(&self) -> SerializerEnum {
        SerializerEnum::from(Self {})
    }

    fn serialize<T: Serialize>(&self, data: &T) -> Option<Vec<u8>> {
        serde_json::to_vec(data).ok()
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(&self, payload: &[u8]) -> Option<T> {
        serde_json::from_slice::<T>(payload).ok()
    }
}

#[cfg_attr(feature = "python", pyclass)]
pub struct KafkaMiddleware {
    #[allow(dead_code)]
    runtime: Arc<tokio::runtime::Runtime>, // Used only in #[pymethods] when python feature is enabled
    admin: OnceLock<AdminClient<DefaultClientContext>>,
    producer: OnceLock<Arc<FutureProducer>>,
    consumers: Mutex<Vec<Arc<StreamConsumer>>>,
    // Temporary producer with specific settings for image publishing.
    // This setup will remain in place until a configurable producer interface is provided to the user.
    image_producer: OnceLock<Arc<FutureProducer>>,
}

impl KafkaMiddleware {
    pub fn new() -> Self {
        KafkaMiddleware {
            runtime: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            admin: OnceLock::new(),
            producer: OnceLock::new(),
            consumers: Mutex::new(Vec::new()),
            image_producer: OnceLock::new(),
        }
    }
}

impl KafkaMiddleware {
    async fn create_topics(&self, topics: &Vec<(String, String)>) {
        let admin = self.admin.get_or_init(|| {
            ClientConfig::new()
                .set("bootstrap.servers", "127.0.0.1:9092")
                .set("broker.address.family", "v4")
                .create()
                .expect("Couldn't create Kafka admin client.")
        });

        let metadata = admin
            .inner()
            .fetch_metadata(None, None)
            .expect("Failed to fetch metadata from Kafka adming client");

        for (_, topic) in topics {
            if !metadata
                .topics()
                .iter()
                .any(|metatopic| metatopic.name() == topic)
            {
                println!("Creating new topic: {}", topic);
                let create_res = admin
                    .create_topics(
                        &[NewTopic {
                            name: topic,
                            num_partitions: 1,
                            replication: TopicReplication::Fixed(1),
                            config: vec![],
                        }],
                        &AdminOptions::default(),
                    )
                    .await
                    .expect("Failed to create topic.");

                for res in create_res {
                    match res {
                        Ok(topic) => {
                            println!("Topic {} created successfully.", topic);
                        }
                        Err((topic, err)) => {
                            println!("Failed to create topic {} with error: {:?}", topic, err);
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl MiddlewareRaw for KafkaMiddleware {
    async fn publish_raw(
        &self,
        _message_type: &str,
        topic: &str,
        payload: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        // let producer = Arc::clone(self.producer.get_or_init(|| {
        //     Arc::new(
        //         ClientConfig::new()
        //             .set("bootstrap.servers", "127.0.0.1:9092")
        //             .set("broker.address.family", "v4")
        //             .set("socket.nagle.disable", "true") // to improve latency perf for many small msgs
        //             .set("linger.ms", "0") // disable batching, send msgs immediately to improve latency
        //             .set("message.timeout.ms", "5000")
        //             .set("enable.idempotence", "true")
        //             .set("acks", "0")
        //             .set("compression.type", "all")
        //             .set("message.max.bytes", "10000000")
        //             .create()
        //             .expect("Couldn't create Kafka producer."),
        //     )
        // }));

        // Temporary image producer with specific settings to improve performance.
        let producer = match _message_type {
            "aerosim::types::CompressedImage" => Arc::clone(self.image_producer.get_or_init(|| {
                Arc::new(
                    ClientConfig::new()
                        .set("bootstrap.servers", "127.0.0.1:9092")
                        .set("broker.address.family", "v4")
                        .set("linger.ms", "0") // disable batching, send msgs immediately to improve latency
                        .set("message.timeout.ms", "5000")
                        .set("acks", "0")
                        .set("compression.type", "none")
                        .set("message.max.bytes", "10000000")
                        .set("debug", "all")
                        .create()
                        .expect("Couldn't create Kafka producer."),
                )
            })),
            _ => Arc::clone(self.producer.get_or_init(|| {
                Arc::new(
                    ClientConfig::new()
                        .set("bootstrap.servers", "127.0.0.1:9092")
                        .set("broker.address.family", "v4")
                        .set("socket.nagle.disable", "true") // to improve latency perf for many small msgs
                        .set("linger.ms", "0") // disable batching, send msgs immediately to improve latency
                        .set("message.timeout.ms", "5000")
                        .set("enable.idempotence", "true")
                        .set("acks", "all")
                        .set("compression.type", "none")
                        .create()
                        .expect("Couldn't create Kafka producer."),
                )
            })),
        };
        producer
            .send(
                FutureRecord::to(topic).key("key").payload(payload),
                Duration::from_secs(0),
            )
            .await
            .map_err(|e| -> Box<dyn Error> {
                format!("Failed to publish topic {} with error: {:?}", topic, e.0).into()
            })?;

        Ok(())
    }

    async fn subscribe_raw(
        &self,
        message_type: &str,
        topic: &str,
        callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        self.subscribe_all_raw(
            vec![(message_type.to_string(), topic.to_string())],
            callback,
        )
        .await
    }

    async fn subscribe_all_raw(
        &self,
        topics: Vec<(String, String)>,
        callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        let consumer: Arc<StreamConsumer> = Arc::new(
            ClientConfig::new()
                .set("bootstrap.servers", "127.0.0.1:9092")
                .set("broker.address.family", "v4")
                .set("socket.nagle.disable", "true") // to improve latency perf for many small msgs
                .set("group.id", "aerosim.simcore")
                .set("client.id", "aerosim.simcore")
                .set("enable.partition.eof", "true")
                .set("enable.auto.commit", "false")
                .set("auto.offset.reset", "latest")
                .set("message.max.bytes", "10000000")
                .set("fetch.message.max.bytes", "10000000")
                .create()
                .expect("Couldn't create Kafka subscriber."),
        );

        // Create missing topics in the Kafka broker.
        self.create_topics(&topics).await;

        {
            let consumer = Arc::clone(&consumer);

            let mut tpl = TopicPartitionList::new();
            for (_, topic) in &topics {
                tpl.add_partition_offset(&topic, 0, rdkafka::Offset::Beginning)
                    .expect("[aerosim.middleware.kafka] Error adding partition offset");
            }

            for ((topic, partition), _) in tpl.to_topic_map() {
                let high_offset =
                    match consumer.fetch_watermarks(&topic, partition, Duration::from_millis(5000))
                    {
                        Ok((_, high_offset)) => high_offset,
                        Err(rdkafka::error::KafkaError::MetadataFetch(
                            rdkafka::types::RDKafkaErrorCode::UnknownPartition,
                        )) => 0,
                        Err(_) => -1,
                    };

                if high_offset >= 0 {
                    tpl.set_partition_offset(
                        &topic,
                        partition,
                        rdkafka::Offset::Offset(high_offset),
                    )
                    .expect("[aerosim.middleware.kafka] Error setting partition offset");
                }
            }

            consumer
                .assign(&tpl)
                .expect("[aerosim.middleware.kafka] Error assigning topics");

            task::spawn(async move {
                let mut stream = consumer.stream();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(sample) => match sample.payload() {
                            Some(payload) => {
                                let _ = callback(payload);
                            }
                            None => println!(
                                "Couldn't extract payload from sample ({})",
                                sample.topic()
                            ),
                        },
                        Err(_) => {}
                    }
                }
            });
        }

        {
            let mut consumers = self.consumers.lock().unwrap();
            consumers.push(consumer);
        }

        Ok(())
    }
}

#[async_trait]
impl Middleware for KafkaMiddleware {
    fn get_serializer(&self) -> SerializerEnum {
        SerializerEnum::from(KafkaSerializer {})
    }
}

#[cfg(feature = "python")]
impl PyMiddleware for KafkaMiddleware {}

#[cfg(feature = "python")]
#[pymethods]
impl KafkaMiddleware {
    #[new]
    fn pynew(_py: Python) -> PyResult<Self> {
        Ok(Self::new())
    }

    #[pyo3(name = "publish")]
    #[pyo3(signature = (topic, message, timestamp_sim=None))]
    fn pypublish(
        &self,
        py: Python,
        topic: &str,
        message: PyObject,
        timestamp_sim: Option<TimeStamp>,
    ) -> PyResult<()> {
        futures::executor::block_on(self.pypublish_impl(py, topic, message, timestamp_sim))
    }

    #[pyo3(name = "subscribe")]
    fn pysubscribe(
        &self,
        py: Python,
        message_type: PyObject,
        topic: &str,
        callback: PyObject,
    ) -> PyResult<()> {
        let handle = self.runtime.handle();
        tokio::task::block_in_place(|| {
            handle.block_on(self.pysubscribe_impl(py, message_type, topic, callback))
        })
    }

    #[pyo3(name = "subscribe_all")]
    fn pysubscribe_all(
        &self,
        py: Python,
        message_type: PyObject,
        topics: Vec<String>,
        callback: PyObject,
    ) -> PyResult<()> {
        let handle = self.runtime.handle();
        tokio::task::block_in_place(|| {
            handle.block_on(self.pysubscribe_all_impl(py, message_type, topics, callback))
        })
    }

    #[pyo3(name = "publish_raw")]
    fn pypublish_raw(
        &self,
        py: Python,
        message_type: &str,
        topic: &str,
        payload: Py<pyo3::types::PyBytes>,
    ) -> PyResult<()> {
        futures::executor::block_on(self.pypublish_raw_impl(py, message_type, topic, payload))
    }

    #[pyo3(name = "subscribe_raw")]
    fn pysubscribe_raw(
        &self,
        py: Python<'_>,
        message_type: &str,
        topic: &str,
        callback: PyObject,
    ) -> PyResult<()> {
        let handle = self.runtime.handle();
        tokio::task::block_in_place(|| {
            handle.block_on(self.pysubscribe_raw_impl(py, message_type, topic, callback))
        })
    }

    #[pyo3(name = "subscribe_all_raw")]
    fn pysubscribe_all_raw(
        &self,
        py: Python,
        topics: Vec<(String, String)>,
        callback: PyObject,
    ) -> PyResult<()> {
        let handle = self.runtime.handle();
        tokio::task::block_in_place(|| {
            handle.block_on(self.pysubscribe_all_raw_impl(py, topics, callback))
        })
    }
}

#[cfg(feature = "python")]
impl PySerializer for KafkaSerializer {}

#[cfg(feature = "python")]
#[pymethods]
impl KafkaSerializer {
    #[new]
    fn pynew(_py: Python) -> PyResult<Self> {
        Ok(Self {})
    }

    #[pyo3(name = "serialize_message")]
    fn pyserialize_message(
        &self,
        py: Python<'_>,
        metadata: Metadata,
        data: PyObject,
    ) -> Option<Vec<u8>> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pyserialize_message_impl(py, &serializer, metadata, data)
    }

    #[pyo3(name = "deserialize_message")]
    fn pydeserialize_message(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<(Metadata, PyObject)> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pydeserialize_message_impl(py, &serializer, message_type, payload)
    }

    #[pyo3(name = "deserialize_metadata")]
    fn pydeserialize_metadata(&self, py: Python<'_>, payload: &[u8]) -> Option<Metadata> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pydeserialize_metadata_impl(py, &serializer, payload)
    }

    #[pyo3(name = "deserialize_data")]
    fn pydeserialize_data(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pydeserialize_data_impl(py, &serializer, message_type, payload)
    }

    #[pyo3(name = "from_json")]
    fn pyserialize_from_json(
        &self,
        py: Python<'_>,
        type_name: &str,
        metadata: &Metadata,
        data: PyObject,
    ) -> Option<Vec<u8>> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pyserialize_from_json_impl(py, &serializer, type_name, metadata, data)
    }

    #[pyo3(name = "to_json")]
    fn pydeserialize_to_json(
        &self,
        py: Python<'_>,
        type_name: &str,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(KafkaSerializer {});
        self.pydeserialize_to_json_impl(py, &serializer, type_name, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: u32,
        name: String,
        value: f64,
    }

    #[test]
    fn test_kafka_serializer_roundtrip() {
        let serializer = KafkaSerializer;
        let original = TestData {
            id: 42,
            name: "test".to_string(),
            value: 3.14,
        };

        let serialized = serializer.serialize(&original);
        assert!(serialized.is_some());

        let deserialized: Option<TestData> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());
        assert_eq!(deserialized.unwrap(), original);
    }

    #[test]
    fn test_kafka_serializer_json_format() {
        let serializer = KafkaSerializer;
        let data = TestData {
            id: 1,
            name: "json_test".to_string(),
            value: 2.5,
        };

        let serialized = serializer.serialize(&data).unwrap();
        let json_str = String::from_utf8(serialized).unwrap();

        // Verify it's valid JSON
        assert!(json_str.contains("\"id\":1"));
        assert!(json_str.contains("\"name\":\"json_test\""));
        assert!(json_str.contains("\"value\":2.5"));
    }

    #[test]
    fn test_kafka_serializer_deserialize_invalid_json() {
        let serializer = KafkaSerializer;
        let invalid_json = b"not valid json{{{";
        let result: Option<TestData> = serializer.deserialize(invalid_json);
        assert!(result.is_none());
    }

    #[test]
    fn test_kafka_serializer_returns_self() {
        let serializer = KafkaSerializer;
        let returned = serializer.serializer();
        match returned {
            SerializerEnum::KafkaSerializer(_) => {}
            _ => panic!("Expected KafkaSerializer variant"),
        }
    }

    #[test]
    fn test_kafka_serializer_serialize_message_with_metadata() {
        let serializer = KafkaSerializer;
        let metadata = Metadata::new("test_topic", "TestData", None, None);
        let data = TestData {
            id: 100,
            name: "message_test".to_string(),
            value: 99.9,
        };

        let payload = serializer.serialize_message(&metadata, &data);
        assert!(payload.is_some());

        let (deserialized_meta, deserialized_data): (Metadata, TestData) =
            serializer.deserialize_message(&payload.unwrap()).unwrap();

        assert_eq!(deserialized_meta.topic, "test_topic");
        assert_eq!(deserialized_meta.type_name, "TestData");
        assert_eq!(deserialized_data, data);
    }

    #[test]
    fn test_kafka_serializer_deserialize_metadata_only() {
        let serializer = KafkaSerializer;
        let metadata = Metadata::new("meta_topic", "MetaType", None, None);
        let data = TestData {
            id: 1,
            name: "test".to_string(),
            value: 1.0,
        };

        let payload = serializer.serialize_message(&metadata, &data).unwrap();
        let deserialized_meta = serializer.deserialize_metadata(&payload);

        assert!(deserialized_meta.is_some());
        let meta = deserialized_meta.unwrap();
        assert_eq!(meta.topic, "meta_topic");
        assert_eq!(meta.type_name, "MetaType");
    }

    #[test]
    fn test_kafka_serializer_deserialize_data_only() {
        let serializer = KafkaSerializer;
        let metadata = Metadata::new("data_topic", "DataType", None, None);
        let data = TestData {
            id: 55,
            name: "data_only".to_string(),
            value: 55.5,
        };

        let payload = serializer.serialize_message(&metadata, &data).unwrap();
        let deserialized_data: Option<TestData> = serializer.deserialize_data(&payload);

        assert!(deserialized_data.is_some());
        assert_eq!(deserialized_data.unwrap(), data);
    }
}
