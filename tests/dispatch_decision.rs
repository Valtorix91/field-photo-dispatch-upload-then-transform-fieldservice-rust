use field_photo_dispatch::work_order_photos::{next_dispatch_status, DispatchStatus};

#[test]
fn technician_follow_up_controls_dispatch() {
    assert_eq!(next_dispatch_status(false), DispatchStatus::Dispatched);
    assert_eq!(
        next_dispatch_status(true),
        DispatchStatus::TechnicianFollowUp
    );
}
