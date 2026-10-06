//! Owns cancellable background reads. Catalog loading does not supersede content.
use super::state::{Payload, Request, RequestKind, Response};
use crate::{
    error::Result,
    service::{Service, issues::IssueFilter},
};
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};
pub async fn execute(service: &Service, kind: RequestKind) -> Result<Payload> {
    match kind {
        RequestKind::Catalog => service.overview_catalog().await.map(Payload::Catalog),
        RequestKind::Projects { filter, page } => service
            .overview_projects(filter, page)
            .await
            .map(Payload::Projects),
        RequestKind::Issues { id, page } => service
            .project_issues(&id, IssueFilter::default(), page)
            .await
            .map(Payload::Issues),
        RequestKind::Issue { id } => service
            .view_issue(&id)
            .await
            .map(|i| Payload::Issue(Box::new(i))),
        RequestKind::Comments { id, page } => service
            .list_comments(&id, page)
            .await
            .map(Payload::Comments),
    }
}
pub struct Loader {
    service: Arc<Service>,
    sender: mpsc::Sender<Response>,
    receiver: mpsc::Receiver<Response>,
    content: Option<JoinHandle<()>>,
    catalog: Option<JoinHandle<()>>,
}
impl Loader {
    pub fn new(service: Arc<Service>) -> Self {
        let (sender, receiver) = mpsc::channel(8);
        Self {
            service,
            sender,
            receiver,
            content: None,
            catalog: None,
        }
    }
    pub fn submit(&mut self, request: Request) {
        let slot = if matches!(request.kind, RequestKind::Catalog) {
            &mut self.catalog
        } else {
            &mut self.content
        };
        if let Some(task) = slot.take() {
            task.abort();
        }
        let service = self.service.clone();
        let sender = self.sender.clone();
        *slot = Some(tokio::spawn(async move {
            let result = execute(&service, request.kind.clone()).await;
            let _ = sender
                .send(Response {
                    generation: request.generation,
                    kind: request.kind,
                    result,
                })
                .await;
        }));
    }
    pub async fn recv(&mut self) -> Option<Response> {
        self.receiver.recv().await
    }
    pub fn cancel_content(&mut self) {
        if let Some(task) = self.content.take() {
            task.abort();
        }
    }
    pub fn cancel(&mut self) {
        self.cancel_content();
        if let Some(task) = self.catalog.take() {
            task.abort();
        }
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
    }
}
