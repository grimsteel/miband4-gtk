use std::collections::HashMap;

use gtk::{glib::{self, Object}, prelude::*, subclass::prelude::*, Accessible, Align, Box as GtkBox, Buildable, Button, ConstraintTarget, Label, Orientable, Orientation, Separator, Widget};

use log::warn;

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
    /// define the item spec for this card
    pub fn handle_items<'a>(&self, items: &'a [InfoItem<'a>]) {
        let mut widget_map = Vec::with_capacity(items.len());
        for (i, InfoItem { item_type, id, label, classes }) in items.iter().enumerate() {
            if i > 0 {
                // add a separator
                let separator = Separator::new(Orientation::Horizontal);
                if item_type != &InfoItemType::Field {
                    // blank separator
                    separator.add_css_class("spacer");
                }
                self.append(&separator);
            }
            
            match item_type {
                InfoItemType::Field => {
                    let field_label = Label::new(Some(label));
                    field_label.set_halign(Align::Start);
                    field_label.add_css_class("dim-label");
                    self.append(&field_label);

                    let value_label = Label::new(None);
                    value_label.set_halign(Align::Start);
                    value_label.add_css_class("title-4");
                    for class in classes.iter() { value_label.add_css_class(class); }
                    self.append(&value_label);

                    widget_map.push(((*id).into(), InfoItemWidget::Field(value_label)));
                },
                InfoItemType::Button => {
                    let button = Button::new();
                    button.set_label(label);
                    for class in classes.iter() { button.add_css_class(class); }
                    button.set_halign(Align::Start);
                    self.append(&button);

                    widget_map.push(((*id).into(), InfoItemWidget::Button(button)));
                },
                InfoItemType::Indicator => {
                    let indicator = Label::new(Some(label));
                    indicator.set_halign(Align::Start);
                    indicator.add_css_class("title-4");
                    for class in classes.iter() { indicator.add_css_class(class); }
                    self.append(&indicator);

                    widget_map.push(((*id).into(), InfoItemWidget::Indicator(indicator)));
                }
            }
        }
        self.imp().items.set(widget_map).expect("cell was not already filled");
    }
    /// set all widgets to loading
    pub fn set_loading(&self) {
        if let Some(items) = self.imp().items.get() {
            for (_id, widget) in items {
                match widget {
                    InfoItemWidget::Field(label) => {
                        // set the value of the field to "loading..."
                        label.set_label("Loading...");
                    },
                    InfoItemWidget::Indicator(label) => {
                        label.set_visible(false);
                    },
                    InfoItemWidget::Button(button) => {
                        button.set_sensitive(true);
                    }
                }
            }
        }
    }
    /// set the values of the widget to the values provided
    pub fn apply_values(&self, values: &HashMap<String, InfoItemValue>) {
        if let Some(items) = self.imp().items.get() {
            for (id, widget) in items {
                // get the corresponding value
                if let Some(value) = values.get(id) {
                    // apply it
                    match (value, widget) {
                        (InfoItemValue::Field(value), InfoItemWidget::Field(label)) => {
                            label.set_label(value);
                        },
                        (InfoItemValue::Indicator(visible), InfoItemWidget::Indicator(label)) => {
                            label.set_visible(*visible);
                        },
                        (InfoItemValue::Button(enabled), InfoItemWidget::Button(button)) => {
                            button.set_sensitive(*enabled);
                        },
                        _ => {
                            // they provided the wrong value type for this widget
                            warn!("value {value:?} has wrong type for widget {widget:?}");
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct InfoItem<'a> {
    pub item_type: InfoItemType,
    pub id: &'a str,
    pub label: &'a str,
    pub classes: &'a[&'a str]
}

#[derive(Debug)]
enum InfoItemWidget {
    Field(Label),
    Indicator(Label),
    Button(Button)
}

/// a single value representing the state
#[derive(Debug)]
pub enum InfoItemValue {
    Field(String),
    Indicator(bool),
    Button(bool)
}

#[derive(Eq, PartialEq, Debug)]
pub enum InfoItemType { Field, Indicator, Button }

mod imp {
    use std::cell::OnceCell;

    use gtk::{glib, prelude::*, subclass::prelude::*, Orientation, Box as GtkBox};

    use super::InfoItemWidget;

    #[derive(Default)]
    pub struct DeviceInfoCard {
        // item ID + enum-widget
        pub items: OnceCell<Vec<(String, InfoItemWidget)>>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceInfoCard {
        const NAME: &'static str = "MiBand4DeviceInfoCard";
        type Type = super::DeviceInfoCard;
        type ParentType = GtkBox;
    }

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
