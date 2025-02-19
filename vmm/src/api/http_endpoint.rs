// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::api::http::EndpointHandler;
use crate::api::{
    vm_boot, vm_create, vm_delete, vm_info, vm_pause, vm_reboot, vm_resume, vm_shutdown,
    vmm_shutdown, ApiError, ApiRequest, ApiResult, VmAction, VmConfig,
};
use micro_http::{Body, Method, Request, Response, StatusCode, Version};
use serde_json::Error as SerdeError;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use vmm_sys_util::eventfd::EventFd;

/// Errors associated with VMM management
#[derive(Debug)]
pub enum HttpError {
    /// API request receive error
    SerdeJsonDeserialize(SerdeError),

    /// Could not create a VM
    VmCreate(ApiError),

    /// Could not boot a VM
    VmBoot(ApiError),

    /// Could not get the VM information
    VmInfo(ApiError),

    /// Could not pause the VM
    VmPause(ApiError),

    /// Could not pause the VM
    VmResume(ApiError),

    /// Could not shut a VM down
    VmShutdown(ApiError),

    /// Could not reboot a VM
    VmReboot(ApiError),

    /// Could not act on a VM
    VmAction(ApiError),

    /// Could not shut the VMM down
    VmmShutdown(ApiError),
}

fn error_response(error: HttpError, status: StatusCode) -> Response {
    let mut response = Response::new(Version::Http11, status);
    response.set_body(Body::new(format!("{:?}", error)));

    response
}

// /api/v1/vm.create handler
pub struct VmCreate {}

impl EndpointHandler for VmCreate {
    fn handle_request(
        &self,
        req: &Request,
        api_notifier: EventFd,
        api_sender: Sender<ApiRequest>,
    ) -> Response {
        match req.method() {
            Method::Put => {
                match &req.body {
                    Some(body) => {
                        // Deserialize into a VmConfig
                        let vm_config: VmConfig = match serde_json::from_slice(body.raw())
                            .map_err(HttpError::SerdeJsonDeserialize)
                        {
                            Ok(config) => config,
                            Err(e) => return error_response(e, StatusCode::BadRequest),
                        };

                        // Call vm_create()
                        match vm_create(api_notifier, api_sender, Arc::new(vm_config))
                            .map_err(HttpError::VmCreate)
                        {
                            Ok(_) => Response::new(Version::Http11, StatusCode::NoContent),
                            Err(e) => error_response(e, StatusCode::InternalServerError),
                        }
                    }

                    None => Response::new(Version::Http11, StatusCode::BadRequest),
                }
            }

            _ => Response::new(Version::Http11, StatusCode::BadRequest),
        }
    }
}

// Common handler for boot, shutdown and reboot
pub struct VmActionHandler {
    action_fn: VmActionFn,
}

type VmActionFn = Box<dyn Fn(EventFd, Sender<ApiRequest>) -> ApiResult<()> + Send + Sync>;

impl VmActionHandler {
    pub fn new(action: VmAction) -> Self {
        let action_fn = Box::new(match action {
            VmAction::Boot => vm_boot,
            VmAction::Delete => vm_delete,
            VmAction::Shutdown => vm_shutdown,
            VmAction::Reboot => vm_reboot,
            VmAction::Pause => vm_pause,
            VmAction::Resume => vm_resume,
        });

        VmActionHandler { action_fn }
    }
}

impl EndpointHandler for VmActionHandler {
    fn handle_request(
        &self,
        req: &Request,
        api_notifier: EventFd,
        api_sender: Sender<ApiRequest>,
    ) -> Response {
        match req.method() {
            Method::Put => {
                match (self.action_fn)(api_notifier, api_sender).map_err(|e| match e {
                    ApiError::VmBoot(_) => HttpError::VmBoot(e),
                    ApiError::VmShutdown(_) => HttpError::VmShutdown(e),
                    ApiError::VmReboot(_) => HttpError::VmReboot(e),
                    ApiError::VmPause(_) => HttpError::VmPause(e),
                    ApiError::VmResume(_) => HttpError::VmResume(e),
                    _ => HttpError::VmAction(e),
                }) {
                    Ok(_) => Response::new(Version::Http11, StatusCode::NoContent),
                    Err(e) => error_response(e, StatusCode::InternalServerError),
                }
            }
            _ => Response::new(Version::Http11, StatusCode::BadRequest),
        }
    }
}

// /api/v1/vm.info handler
pub struct VmInfo {}

impl EndpointHandler for VmInfo {
    fn handle_request(
        &self,
        req: &Request,
        api_notifier: EventFd,
        api_sender: Sender<ApiRequest>,
    ) -> Response {
        match req.method() {
            Method::Get => match vm_info(api_notifier, api_sender).map_err(HttpError::VmInfo) {
                Ok(info) => {
                    let mut response = Response::new(Version::Http11, StatusCode::OK);
                    let info_serialized = serde_json::to_string(&info).unwrap();

                    response.set_body(Body::new(info_serialized));
                    response
                }
                Err(e) => error_response(e, StatusCode::InternalServerError),
            },
            _ => Response::new(Version::Http11, StatusCode::BadRequest),
        }
    }
}

// /api/v1/vmm.shutdown handler
pub struct VmmShutdown {}

impl EndpointHandler for VmmShutdown {
    fn handle_request(
        &self,
        req: &Request,
        api_notifier: EventFd,
        api_sender: Sender<ApiRequest>,
    ) -> Response {
        match req.method() {
            Method::Put => {
                match vmm_shutdown(api_notifier, api_sender).map_err(HttpError::VmmShutdown) {
                    Ok(_) => Response::new(Version::Http11, StatusCode::OK),
                    Err(e) => error_response(e, StatusCode::InternalServerError),
                }
            }
            _ => Response::new(Version::Http11, StatusCode::BadRequest),
        }
    }
}
