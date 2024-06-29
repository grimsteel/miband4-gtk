use gtk::{glib::{self, Object}, subclass::prelude::ObjectSubclassIsExt, Accessible, Box as GtkBox, Buildable, ConstraintTarget, Orientable, Widget};

glib::wrapper! {
    pub struct DeviceInfoCard(ObjectSubclass<imp::DeviceInfoCard>)
        // https://docs.gtk.org/gtk4/class.Box.html#hierarchy
        @extends GtkBox, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Orientable;
}

impl DeviceInfoCard {
    pub fn new(items: &[InfoItem]) -> Self {
        let object: Self = Object::builder().build();
        object.imp.handle_items(items);
    }
}

pub enum InfoItem {
    Field { label: String, id: String },
    Button { label: String, id: String }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, Object, Properties}, prelude::*, subclass::prelude::*, Align, Box as GtkBox, Label, Orientation, Widget};

    use super::InfoItem;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::DeviceInfoCard)]
    pub struct DeviceInfoCard {
        #[property(get, set)]
        // the properties of this object correspond with the `id`s of the InfoItems.
        // the values of the properties are strings/numbers (for Fields) or bools (for Buttons)
        pub values: RefCell<Option<Object>>,
    }

    impl DeviceInfoCard {
        pub(super) fn handle_items(&self, items: &[InfoItem]) {
            let values = self.obj().property_expression("values");
            for item in items {
                match item {
                    InfoItem::Field { label, id } => {
                        let field_label = Label::new(Some(label));
                        field_label.set_halign(Align::Start);
                        field_label.add_css_class("dim-label");
                        self.obj().append(&field_label);

                        let value_label = Label::new(None);
                        value_label.set_halign(Align::Start);
                        value_label.add_css_class("title-4");
                        self.obj().append(&value_label);

                        // bind the contents
                        values.chain_property::<Object>(id)
                            .bind(&value_label, "label", Widget::NONE);
                    },
                    InfoItem::Button { label, id } => {}
                }
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceInfoCard {
        const NAME: &'static str = "MiBand4DeviceInfoCard";
        type Type = super::DeviceInfoCard;
        type ParentType = GtkBox;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceInfoCard {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_orientation(Orientation::Vertical);
            obj.add_css_class("card");
            obj.add_css_class("device-info-card");
        }
    }

    impl WidgetImpl for DeviceInfoCard {}
    impl BoxImpl for DeviceInfoCard {}
}
