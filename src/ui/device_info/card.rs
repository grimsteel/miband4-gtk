use gtk::{glib::{self, Object}, subclass::prelude::ObjectSubclassIsExt, Accessible, Box as GtkBox, Buildable, ConstraintTarget, Orientable, Widget};

glib::wrapper! {
    pub struct DeviceInfoCard(ObjectSubclass<imp::DeviceInfoCard>)
        // https://docs.gtk.org/gtk4/class.Box.html#hierarchy
        @extends GtkBox, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Orientable;
}

impl DeviceInfoCard {
    pub fn new() -> Self {
        Object::builder().build()
    }
    pub fn handle_items<'a>(&self, items: &'a [InfoItem<'a>]) {
        self.imp().handle_items(items);
    }
}

pub struct InfoItem<'a> {
    pub item_type: InfoItemType,
    pub id: &'a str,
    pub label: &'a str,
    pub classes: &'a[&'a str]
}

#[derive(Eq, PartialEq)]
pub enum InfoItemType { Field, Indicator, Button }

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, Object, Properties}, prelude::*, subclass::prelude::*, Align, Box as GtkBox, Button, Label, Orientation, Separator, Widget};

    use super::{InfoItem, InfoItemType};

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::DeviceInfoCard)]
    pub struct DeviceInfoCard {
        #[property(get, set)]
        // the properties of this object correspond with the `id`s of the InfoItems.
        // the values of the properties are strings/numbers (for Fields) or bools (for Buttons/Indicators)
        pub values: RefCell<Option<Object>>,
    }

    impl DeviceInfoCard {
        pub(super) fn handle_items<'a>(&self, items: &'a [InfoItem<'a>]) {
            let values = self.obj().property_expression("values");
            for (i, InfoItem { item_type, id, label, classes }) in items.iter().enumerate() {
                if i > 0 {
                    // add a separator
                    let separator = Separator::new(Orientation::Horizontal);
                    if item_type != &InfoItemType::Field {
                        // blank separator
                        separator.add_css_class("spacer");
                    }
                    self.obj().append(&separator);
                }
                
                match item_type {
                    InfoItemType::Field => {
                        let field_label = Label::new(Some(label));
                        field_label.set_halign(Align::Start);
                        field_label.add_css_class("dim-label");
                        self.obj().append(&field_label);

                        let value_label = Label::new(None);
                        value_label.set_halign(Align::Start);
                        value_label.add_css_class("title-4");
                        for class in classes.iter() { value_label.add_css_class(class); }
                        self.obj().append(&value_label);

                        // bind the contents
                        values.chain_property::<Object>(id)
                            .bind(&value_label, "label", Widget::NONE);
                    },
                    InfoItemType::Button => {
                        let button = Button::new();
                        button.set_label(label);
                        for class in classes.iter() { button.add_css_class(class); }
                        button.set_halign(Align::Start);
                        self.obj().append(&button);

                        values.chain_property::<Object>(id)
                            .bind(&button, "sensitive", Widget::NONE);
                    },
                    InfoItemType::Indicator => {
                        let indicator = Label::new(Some(label));
                        indicator.set_halign(Align::Start);
                        indicator.add_css_class("title-4");
                        for class in classes.iter() { indicator.add_css_class(class); }
                        self.obj().append(&indicator);

                        values.chain_property::<Object>(id)
                            .bind(&indicator, "visible", Widget::NONE);
                    }
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
