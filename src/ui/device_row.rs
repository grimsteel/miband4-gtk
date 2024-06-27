use gtk::{glib::{self, Object}, Accessible, Box as GtkBox, Buildable, ConstraintTarget, Orientable, Widget};


glib::wrapper! {
    pub struct DeviceRow(ObjectSubclass<imp::DeviceRow>)
        @extends GtkBox, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Orientable;
}

impl DeviceRow {
    pub fn new() -> Self {
        Object::builder().build()
    }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, subclass::InitializingObject, Properties}, prelude::*, subclass::prelude::*, Box as GtkBox, CompositeTemplate, Label, Widget};

    use crate::ui::device_row_object::DeviceRowObject;

    #[derive(Properties, Default, CompositeTemplate)]
    #[template(resource = "/me/grimsteel/miband4-gtk/device_row.ui")]
    #[properties(wrapper_type = super::DeviceRow)]
    pub struct DeviceRow {
        #[property(get, set)]
        device: RefCell<Option<DeviceRowObject>>,
        #[template_child]
        pub address_label: TemplateChild<Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceRow {
        const NAME: &'static str = "MiBand4DeviceRow";
        type Type = super::DeviceRow;
        type ParentType = GtkBox;

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
            // bind obj->device->address to address_label->label
            obj.property_expression("device")
                .chain_property::<DeviceRowObject>("address")
                .bind(&self.address_label.get(), "label", Widget::NONE);
        }
    }

    impl WidgetImpl for DeviceRow {}

    impl BoxImpl for DeviceRow {}
}
