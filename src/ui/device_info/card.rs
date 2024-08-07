use std::collections::HashMap;

use gtk::{glib::{self, clone, Object}, pango::EllipsizeMode, prelude::*, subclass::prelude::*, Accessible, Align, Box as GtkBox, Buildable, Button, ConstraintTarget, Entry, Label, Orientable, Orientation, Separator, Switch, Widget};

use log::warn;

use super::card_implementations::IntoInfoItemValues;

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
                if item_type == &InfoItemType::Button || item_type == &InfoItemType::Indicator {
                    // blank separator
                    separator.add_css_class("spacer");
                }
                self.append(&separator);
            }

            let id: String = (*id).into();
            
            match item_type {
                InfoItemType::Field => {
                    // a label for this field
                    let field_label = Label::new(Some(label));
                    field_label.set_halign(Align::Start);
                    field_label.add_css_class("dim-label");
                    self.append(&field_label);

                    // the actual field value
                    let value_label = Label::new(None);
                    value_label.set_halign(Align::Start);
                    value_label.set_ellipsize(EllipsizeMode::End);
                    value_label.add_css_class("title-4");
                    for class in classes.iter() { value_label.add_css_class(class); }
                    
                    self.append(&value_label);
                    widget_map.push((id, InfoItemWidget::Field(value_label)));
                },
                InfoItemType::Button => {
                    let button = Button::new();
                    button.set_label(label);
                    for class in classes.iter() { button.add_css_class(class); }
                    button.set_halign(Align::Start);

                    // connect the event listener
                    button.connect_clicked(clone!(@weak self as win, @strong id => move |_button| {
                        win.emit_by_name::<()>("button-clicked", &[&id]);
                    }));
                    
                    self.append(&button);
                    widget_map.push((id, InfoItemWidget::Button(button)));
                },
                InfoItemType::Indicator => {
                    let indicator = Label::new(Some(label));
                    indicator.set_halign(Align::Start);
                    indicator.add_css_class("title-4");
                    for class in classes.iter() { indicator.add_css_class(class); }
                    
                    self.append(&indicator);
                    widget_map.push((id, InfoItemWidget::Indicator(indicator)));
                },
                InfoItemType::Switch => {
                    // a label for this switch
                    let field_label = Label::new(Some(label));
                    field_label.set_halign(Align::Start);
                    field_label.add_css_class("dim-label");
                    self.append(&field_label);

                    // the toggle siwtch
                    let switch = Switch::new();
                    switch.set_halign(Align::Start);

                    self.append(&switch);
                    widget_map.push((id, InfoItemWidget::Switch(switch)));
                },
                InfoItemType::Entry => {
                    // a label for this entry
                    let field_label = Label::new(Some(label));
                    field_label.set_halign(Align::Start);
                    field_label.add_css_class("dim-label");
                    self.append(&field_label);

                    // the text entry
                    let entry = Entry::new();

                    self.append(&entry);
                    widget_map.push((id, InfoItemWidget::Entry(entry)));
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
                        button.set_sensitive(false);
                    },
                    InfoItemWidget::Switch(switch) => {
                        switch.set_sensitive(false);
                    },
                    InfoItemWidget::Entry(entry) => {
                        entry.set_sensitive(false);
                    }
                }
            }
        }
    }
    /// set the values of the widget to the values provided
    pub fn apply_values<T: IntoInfoItemValues>(&self, values: T) {
        let values = values.into_info_item_values();
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
                        (InfoItemValue::Switch(checked), InfoItemWidget::Switch(switch)) => {
                            switch.set_sensitive(true);
                            switch.set_active(*checked);
                        },
                        (InfoItemValue::Entry(contents), InfoItemWidget::Entry(entry)) => {
                            entry.set_sensitive(true);
                            entry.buffer().set_text(contents);
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
    /// get the value of the switches and entries
    pub fn get_values(&self) -> InfoItemValues {
        if let Some(items) = self.imp().items.get() {
            items.iter().filter_map(|item| {
                match item {
                    (id, InfoItemWidget::Switch(switch)) => Some((id.clone(), InfoItemValue::Switch(switch.is_active()))),
                    (id, InfoItemWidget::Entry(entry)) => Some((id.clone(), InfoItemValue::Entry(entry.buffer().text().as_str().to_string()))),
                    _ => None
                }
            }).collect()
        } else {
            HashMap::new()
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
    Button(Button),
    Switch(Switch),
    Entry(Entry)
}

/// a single value representing the state
#[derive(Debug)]
pub enum InfoItemValue {
    Field(String),
    Indicator(bool),
    Button(bool),
    Switch(bool),
    Entry(String)
}

#[derive(Eq, PartialEq, Debug)]
pub enum InfoItemType { Field, Indicator, Button, Switch, Entry }

pub type InfoItemValues = HashMap<String, InfoItemValue>;

mod imp {
    use std::{cell::OnceCell, sync::OnceLock};

    use gtk::{glib::{self, subclass::Signal}, prelude::*, subclass::prelude::*, Box as GtkBox, Orientation};

    use super::InfoItemWidget;

    #[derive(Default)]
    pub struct DeviceInfoCard {
        // item ID + enum-widget
        pub(super) items: OnceCell<Vec<(String, InfoItemWidget)>>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceInfoCard {
        const NAME: &'static str = "MiBand4DeviceInfoCard";
        type Type = super::DeviceInfoCard;
        type ParentType = GtkBox;
    }

    impl ObjectImpl for DeviceInfoCard {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("button-clicked")
                        // param is the id of the button
                        .param_types([String::static_type()])
                        .build()
                ]
            })
        }
        
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
