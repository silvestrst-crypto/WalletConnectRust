use {
    anyhow::Result,
    chrono::Utc,
    clap::Parser,
    dashmap::DashMap,
    relay_client::{
        error::Error,
        websocket::{Client, CloseFrame, ConnectionHandler, PublishedMessage},
        ConnectionOptions,
    },
    relay_rpc::{
        auth::{ed25519_dalek::Keypair, rand, AuthToken},
        domain::{SubscriptionId, Topic},
    },
    sign_api::{
        crypto::{
            payload::{decode_and_decrypt_type0, encrypt_and_encode, EnvelopeType},
            session::SessionKey,
        },
        pairing_uri::Pairing,
        rpc::*,
    },
    std::str::FromStr,
    std::sync::Arc,
    tokio::{
        select,
        sync::mpsc::{channel, unbounded_channel, Sender, UnboundedSender},
        time::Duration,
    },
};

const SUPPORTED_PROTOCOL: &str = "irn";
const SUPPORTED_METHODS: &[&str] = &[
    "eth_sendTransaction",
    "eth_signTransaction",
    "eth_sign",
    "personal_sign",
    "eth_signTypedData",
];
const SUPPORTED_EVENTS: &[&str] = &["chainChanged", "accountsChanged"];

// Establish Session.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Arg {
    /// Goerli https://react-app.walletconnect.com/ pairing URI.
    pairing_uri: String,

    /// Specify WebSocket address.
    #[arg(short, long, default_value = "wss://relay.walletconnect.com")]
    address: String,

    /// Specify WalletConnect project ID.
    #[arg(short, long, default_value = "3cbaa32f8fbf3cdcc87d27ca1fa68069")]
    project_id: String,
}

struct Handler {
    name: &'static str,
    sender: UnboundedSender<PublishedMessage>,
}

impl Handler {
    fn new(name: &'static str, sender: UnboundedSender<PublishedMessage>) -> Self {
        Self { name, sender }
    }
}

impl ConnectionHandler for Handler {
    fn connected(&mut self) {
        println!("\n[{}] connection open", self.name);
    }

    fn disconnected(&mut self, frame: Option<CloseFrame<'static>>) {
        println!("\n[{}] connection closed: frame={frame:?}", self.name);
    }

    fn message_received(&mut self, message: PublishedMessage) {
        println!(
            "\n[{}] inbound message: message_id={} topic={} tag={} message={}",
            self.name, message.message_id, message.topic, message.tag, message.message,
        );

        if let Err(e) = self.sender.send(message) {
            println!("\n[{}] failed to send the to the receiver: {e}", self.name);
        }
    }

    fn inbound_error(&mut self, error: Error) {
        println!("\n[{}] inbound error: {error}", self.name);
    }

    fn outbound_error(&mut self, error: Error) {
        println!("\n[{}] outbound error: {error}", self.name);
    }
}

fn create_conn_opts(address: &str, project_id: &str) -> ConnectionOptions {
    let key = Keypair::generate(&mut rand::thread_rng());

    let auth = AuthToken::new("http://example.com")
        .aud(address)
        .ttl(Duration::from_secs(60 * 60))
        .as_jwt(&key)
        .unwrap();

    ConnectionOptions::new(project_id, auth).with_address(address)
}

fn supported_propose_namespaces() -> Namespaces {
    Namespaces {
        eip155: Some(Namespace {
            chains: vec!["eip155:1".to_string(), "eip155:5".to_string()],
            methods: SUPPORTED_METHODS.iter().map(|m| m.to_string()).collect(),
            events: SUPPORTED_EVENTS.iter().map(|e| e.to_string()).collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn supported_settle_namespaces(account: String) -> SettleNamespaces {
    SettleNamespaces {
        eip155: Some(SettleNamespace {
            accounts: vec![account],
            methods: SUPPORTED_METHODS.iter().map(|m| m.to_string()).collect(),
            events: SUPPORTED_EVENTS.iter().map(|e| e.to_string()).collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Provides a random account information from Goerli explorer.
fn supported_account() -> String {
    "eip155:5:0xBA5BA3955463ADcc7aa3E33bbdfb8A68e0933dD8".to_string()
}

fn create_settle_request(responder_public_key: String) -> RequestParam {
    RequestParam::SessionSettle(SessionSettleRequest {
        relay: Relay {
            protocol: SUPPORTED_PROTOCOL.to_string(),
            data: None,
        },
        controller: Controller {
            public_key: responder_public_key.to_string(),
            metadata: Metadata {
                name: format!("Rust session example: {}", Utc::now()),
                icons: vec!["https://www.rust-lang.org/static/images/rust-logo-blk.svg".to_string()],
                ..Default::default()
            },
        },
        namespaces: supported_settle_namespaces(supported_account()),
        expiry: 300000000000, // 5 min in uSec
    })
}

fn create_proposal_response(responder_public_key: String) -> ResponseParamSuccess {
    ResponseParamSuccess::SessionPropose(SessionProposeResponse {
        relay: Relay {
            protocol: SUPPORTED_PROTOCOL.to_string(),
            data: None,
        },
        responder_public_key,
    })
}

/// https://specs.walletconnect.com/2.0/specs/clients/sign/session-proposal
async fn process_proposal_request(
    context: &Context,
    proposal: SessionProposeRequest,
) -> Result<ResponseParamSuccess> {
    supported_propose_namespaces().supported(&proposal.required_namespaces)?;

    let sender_public_key = hex::decode(&proposal.proposer.public_key)?
        .as_slice()
        .try_into()?;

    let session_key = SessionKey::from_osrng(&sender_public_key)?;
    let responder_public_key = hex::encode(session_key.diffie_public_key());
    let session_topic: Topic = session_key
        .generate_topic()
        .try_into()?;

    let subscription_id = context.client.subscribe(session_topic.clone()).await?;
    _ = context.sessions.insert(
        session_topic.clone(),
        Session { session_key, subscription_id },
    );

    let settle_params = create_settle_request(responder_public_key.clone());
    context
        .publish_request(session_topic, settle_params)
        .await?;

    Ok(create_proposal_response(responder_public_key))
}

fn process_session_delete_request(delete_params: SessionDeleteRequest) -> ResponseParamSuccess {
    println!(
        "\nSession is being terminated reason={}, code={}",
        delete_params.message, delete_params.code,
    );

    ResponseParamSuccess::SessionDelete(true)
}

async fn process_inbound_request(
    context: &Context,
    request: Request,
    topic: Topic,
) -> Result<()> {
    let mut session_delete_cleanup_required: Option<Topic> = None;
    let response = match request.params {
        RequestParam::SessionPropose(proposal) => {
            process_proposal_request(context, proposal).await?
        }
        RequestParam::SessionDelete(params) => {
            session_delete_cleanup_required = Some(topic.clone());
            process_session_delete_request(params)
        }
        RequestParam::SessionPing(_) => ResponseParamSuccess::SessionPing(true),
        _ => todo!(),
    };

    context
        .publish_success_response(topic, request.id, response)
        .await?;

    // Corner case after the session was closed by the dapp.
    if let Some(topic) = session_delete_cleanup_required {
        context.session_delete_cleanup(topic).await?
    }

    Ok(())
}

fn process_inbound_response(response: Response) -> Result<()> {
    match response.param {
        ResponseParam::Success(value) => {
            let params = serde_json::from_value::<ResponseParamSuccess>(value)?;
            match params {
                ResponseParamSuccess::SessionSettle(b)
                | ResponseParamSuccess::SessionDelete(b)
                | ResponseParamSuccess::SessionPing(b) => {
                    if !b {
                        anyhow::bail!("Unsuccessful response={params:?}");
                    }

                    Ok(())
                }
                _ => todo!(),
            }
        }
        ResponseParam::Err(value) => {
            let params = serde_json::from_value::<ResponseParamError>(value)?;
            anyhow::bail!("DApp send and error response: {params:?}");
        }
    }
}

async fn process_inbound_message(context: &Context, message: PublishedMessage) -> Result<()> {
    let plain = context.peek_sym_key(&message.topic, |key| {
        decode_and_decrypt_type0(message.message.as_bytes(), key)
    })?;

    println!("\nPlain payload={plain}");
    let payload: Payload = serde_json::from_str(&plain)?;

    match payload {
        Payload::Request(request) => {
            process_inbound_request(context, request, message.topic).await
        }
        Payload::Response(response) => {
            process_inbound_response(response)
        },
    }
}

async fn inbound_handler(context: Arc<Context>, message: PublishedMessage) {
    if !Payload::irn_tag_in_range(message.tag) {
        println!(
            "\ntag={} skip handling, doesn't belong to Sign API",
            message.tag
        );
        return;
    }

    match process_inbound_message(&context, message).await {
        Ok(_) => println!("\nMessage was successfully handled"),
        Err(e) => println!("\nFailed to handle the message={e}"),
    }
}

struct Connection {
    terminator: Sender<()>,
    topic: Topic,
    subscription_id: SubscriptionId,
    sym_key: [u8; 32],
}

struct Session {
    subscription_id: SubscriptionId,
    session_key: SessionKey,
}

/// Complete pairing context.
struct Context {
    client: Client,
    pairing: Connection,
    sessions: DashMap<Topic, Session>,
}

impl Context {
    fn new(client: Client, pairing: Connection) -> Arc<Self> {
        Arc::new(Self {
            client,
            pairing,
            sessions: DashMap::new(),
        })
    }

    /// Provides read access to the symmetric encryption/decryption key.
    ///
    /// Read lock is held for the duration of the call.
    fn peek_sym_key<F, T>(&self, topic: &Topic, f: F) -> Result<T>
    where
        F: FnOnce(&[u8; 32]) -> Result<T>,
    {
        if &self.pairing.topic == topic {
            f(&self.pairing.sym_key)
        } else {
            let session = self
                .sessions
                .get(topic)
                .ok_or_else(|| anyhow::anyhow!("Missing sym key for topic={} ", topic))?;

            f(&session.session_key.symmetric_key())
        }
    }

    async fn publish_request(&self, topic: Topic, params: RequestParam) -> Result<()> {
        let irn_helpers = params.irn_metadata();
        let request = Request::new(params);
        let payload = serde_json::to_string(&Payload::from(request))?;
        println!("\nSending request topic={topic} payload={payload}");
        self.publish_payload(topic, irn_helpers, &payload).await
    }

    async fn publish_success_response(
        &self,
        topic: Topic,
        id: u64,
        params: ResponseParamSuccess,
    ) -> Result<()> {
        let irn_metadata = params.irn_metadata();
        let response = Response::new(id, params.try_into()?);
        let payload = serde_json::to_string(&Payload::from(response))?;
        println!("\nSending response topic={topic} payload={payload}");
        self.publish_payload(topic, irn_metadata, &payload).await
    }

    async fn publish_payload(
        &self,
        topic: Topic,
        irn_metadata: IrnMetadata,
        payload: &str,
    ) -> Result<()> {
        let encrypted = self.peek_sym_key(&topic, |key| {
            encrypt_and_encode(EnvelopeType::Type0, &payload, key)
        })?;

        println!("\nOutbound encrypted payload={encrypted}");

        self.client
            .publish(
                topic,
                Arc::from(encrypted),
                irn_metadata.tag,
                Duration::from_secs(irn_metadata.ttl),
                irn_metadata.prompt,
            )
            .await?;

        Ok(())
    }

    async fn session_delete_cleanup(&self, topic: Topic) -> Result<()> {
        let (topic, session) = self
            .sessions
            .remove(&topic)
            .ok_or_else(|| anyhow::anyhow!("Attempt to remove non-existing session"))?;

        self.client
            .unsubscribe(topic, session.subscription_id)
            .await?;

        // Un-pair when there are no more session subscriptions.
        if self.sessions.is_empty() {
            println!("\nNo active sessions left, terminating the pairing");

            self.client
                .unsubscribe(
                    self.pairing.topic.clone(),
                    self.pairing.subscription_id.clone(),
                )
                .await?;

            self.pairing.terminator.send(()).await?;
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arg::parse();
    let pairing = Pairing::from_str(&args.pairing_uri)?;
    let topic: Topic = pairing.topic.try_into()?;
    let (inbound_sender, mut inbound_receiver) = unbounded_channel();
    let (terminate_sender, mut terminate_receiver) = channel::<()>(1);

    let client = Client::new(Handler::new("example_wallet", inbound_sender));
    client
        .connect(&create_conn_opts(&args.address, &args.project_id))
        .await?;

    let subscription_id = client.subscribe(topic.clone()).await?;
    println!("\n[client1] subscribed: topic={topic} subscription_id={subscription_id}");

    let context = Context::new(
        client,
        Connection {
            terminator: terminate_sender,
            topic,
            sym_key: pairing.params.sym_key.as_slice().try_into()?,
            subscription_id,
        },
    );

    // Processes inbound messages until termination signal is received.
    loop {
        let context = context.clone();
        select! {
            message = inbound_receiver.recv() => {
                match message {
                    Some(m) => {
                        tokio::spawn(async move { inbound_handler(context, m).await });
                    },
                    None => {
                        break;
                    }
                }

            }
            _ = terminate_receiver.recv() => {
                terminate_receiver.close();
                inbound_receiver.close();
            }
        };
    }

    Ok(())
}
