use gtk::{glib::{self, Object}, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager, Widget, Window};

glib::wrapper! {
    pub struct AuthKeyDialog(ObjectSubclass<imp::AuthKeyDialog>)
        // https://docs.gtk.org/gtk4/class.Window.html#hierarchy
        @extends Window, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl AuthKeyDialog {
    pub fn new() -> Self {
        Object::builder().build()
    }
}

mod imp {
    use std::{cell::RefCell, sync::OnceLock};

    use gtk::{glib::{self, subclass::{InitializingObject, Signal}, Properties}, prelude::{GObjectPropertyExpressionExt, ObjectExt, StaticType}, subclass::prelude::*, template_callbacks, Button, CompositeTemplate, Entry, TemplateChild, Widget, Window};

    use crate::utils::is_hex_string;

    #[derive(CompositeTemplate, Default, Properties)]
    #[template(resource = "/me/grimsteel/miband4-gtk/auth_key_dialog.ui")]
    #[properties(wrapper_type = super::AuthKeyDialog)]
    pub struct AuthKeyDialog {
        #[template_child]
        entry_auth_key: TemplateChild<Entry>,
        #[property(get, set)]
        pub auth_key: RefCell<String>
    }

    #[template_callbacks]
    impl AuthKeyDialog {
        #[template_callback]
        fn handle_auth_key_cancel(&self, _button: &Button) {
            self.obj().emit_by_name::<()>("closed", &[]);
        }
        #[template_callback]
        fn handle_auth_key_save(&self, _button: &Button) {
            // validate the value of the input
            let value = self.obj().auth_key();
            if value.len() == 32 && is_hex_string(&value) {
                self.obj().emit_by_name::<()>("confirmed", &[&value]);
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AuthKeyDialog {
        const NAME: &'static str = "MiBand4AuthKeyDialog";
        type Type = super::AuthKeyDialog;
        type ParentType = Window;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AuthKeyDialog {
        fn constructed(&self) {
            self.parent_constructed();
            
            self.obj().property_expression("auth_key").bind(&self.entry_auth_key.get(), "buffer", Widget::NONE);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("closed").build(),
                    Signal::builder("confirmed").param_types([String::static_type()]).build()
                ]
            })
        }
    }
    impl WidgetImpl for AuthKeyDialog {}
    impl WindowImpl for AuthKeyDialog {}
}
