/// Inter-process message handler

use super::MessageHandler;
use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::permissions::validator::PermissionValidator;
use crate::state::HyperwalletState;
use hyperware_process_lib::logging::{error, info};
use hyperware_process_lib::{Address, Response};

pub struct ProcessHandler {
    permission_validator: PermissionValidator,
}

impl ProcessHandler {
    pub fn new() -> Self {
        Self {
            permission_validator: PermissionValidator::new(),
        }
    }

    fn handle_operation_request(
        &self,
        source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        // Try to parse as an OperationRequest
        let operation_request: OperationRequest = match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse process request from {}: {}", source, e);
                let error = OperationError::invalid_params(&format!("Invalid request format: {}", e));
                let response = OperationResponse::error(error);
                Response::new()
                    .body(serde_json::to_vec(&response)?)
                    .send()?;
                return Ok(());
            }
        };

        // Validate that the auth matches the source
        let source_address = source.to_string();
        if operation_request.auth.process_address != source_address {
            error!(
                "Auth mismatch: claimed {} but source is {}",
                operation_request.auth.process_address, source_address
            );
            let error = OperationError::authentication_failed(
                "Process address in auth does not match message source",
            );
            let response = OperationResponse::error(error);
            Response::new()
                .body(serde_json::to_vec(&response)?)
                .send()?;
            return Ok(());
        }

        info!(
            "Process request from {}: {:?}",
            source, operation_request.operation
        );

        // Execute the operation with permission validation
        let response = self.permission_validator.execute_with_permissions(
            operation_request,
            &source_address,
            state,
        );

        // Send response back to the requesting process
        Response::new()
            .body(serde_json::to_vec(&response)?)
            .send()?;

        Ok(())
    }
}

impl MessageHandler for ProcessHandler {
    fn handle(
        &self,
        source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        self.handle_operation_request(source, body, state)
    }
}
