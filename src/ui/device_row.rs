use gtk::{glib::{self, Object}, Accessible, Grid, Buildable, ConstraintTarget, Orientable, Widget};


glib::wrapper! {
    pub struct DeviceRow(ObjectSubclass<imp::DeviceRow>)
        @extends Grid, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Orientable;
}

impl DeviceRow {
    pub fn new() -> Self {
        Object::builder().build()
    }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, closure, subclass::InitializingObject, Properties, Object}, prelude::*, subclass::prelude::*, Grid, CompositeTemplate, Label, Widget};

    use crate::ui::device_row_object::DeviceRowObject;

    #[derive(Properties, Default, CompositeTemplate)]
    #[template(resource = "/me/grimsteel/miband4-gtk/device_row.ui")]
    #[properties(wrapper_type = super::DeviceRow)]
    pub struct DeviceRow {
        #[property(get, set)]
        device: RefCell<Option<DeviceRowObject>>,
        #[template_child]
        pub address_label: TemplateChild<Label>,
        #[template_child]
        pub rssi_label: TemplateChild<Label>,
        #[template_child]
        pub connected_label: TemplateChild<Label>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceRow {
        const NAME: &'static str = "MiBand4DeviceRow";
        type Type = super::DeviceRow;
        type ParentType = Grid;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceRow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let device = obj.property_expression("device");
            // bind obj->device->address to address_label->label
            device.chain_property::<DeviceRowObject>("alias") // it should be called "alias_label" but I don't feel like changing everything
                .bind(&self.address_label.get(), "label", Widget::NONE);

            device.chain_property::<DeviceRowObject>("rssi")
                .chain_closure::<String>(closure!(|_: Option<Object>, rssi: i32| {
                    if rssi == 0 { "RSSI: ?".into() } else { format!("RSSI: {rssi}") }
                }))
                .bind(&self.rssi_label.get(), "label", Widget::NONE);
            
            device.chain_property::<DeviceRowObject>("connected")
                .bind(&self.connected_label.get(), "visible", Widget::NONE);
                
        }
    }

    impl WidgetImpl for DeviceRow {}

    impl GridImpl for DeviceRow {}
}
