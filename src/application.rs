use crate::config;
use crate::deps::*;
// TODO: uncomment this
// use crate::database::BookxLibrary;
use crate::settings::{settings_manager, Key};
use crate::ui::{BookxView, BookxWindow};

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use glib::{clone, ObjectExt, ParamSpec, ParamSpecObject, Receiver, Sender, ToValue};
use gtk::subclass::application::GtkApplicationImpl;
use gtk_macros::*;
use log::{debug, error, info};
use once_cell::sync::{Lazy, OnceCell};

use adw::prelude::*;
use adw::subclass::prelude::*;

#[derive(Debug, Clone)]
pub enum Action {
    SettingsKeyChanged(Key),
}

mod imp {
    use super::*;
    use once_cell::sync::OnceCell;

    // The basic struct that holds our
    // state and widgets
    pub struct BookxApplication {
        pub sender: Sender<Action>,
        pub receiver: RefCell<Option<Receiver<Action>>>,

        pub window: OnceCell<glib::WeakRef<BookxWindow>>,
        // TODO: uncomment this
        // pub library: BookxLibrary,
        pub settings: gio::Settings,
    }

    // Sets up the basics for the GObject
    // The `#[glib::object_subclass] macro implements
    // some boilerplate code for the object setup, e.g. get_type()
    #[glib::object_subclass]
    impl ObjectSubclass for BookxApplication {
        const NAME: &'static str = "BookxApplication";
        type Type = super::BookxApplication;
        type ParentType = adw::Application;

        fn new() -> Self {
            let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            let receiver = RefCell::new(Some(r));

            let window = OnceCell::new();
            // TODO: uncomment this
            // let library = BookxLibrary::new(sender.clone());

            let settings = settings_manager::settings();

            Self {
                sender,
                receiver,
                window,
                // library,
                settings,
            }
        }
    }

    // Overrides GObject vfuncs
    impl ObjectImpl for BookxApplication {
        // fn constructed(&self, obj: &Self::Type) {
        //     self.parent_constructed(obj);

        // Force dark theme
        //     obj.style_manager()
        //         .set_color_scheme(adw::ColorScheme::PreferDark);

        // Set up the actions
        //     obj.setup_gactions();
        // }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecObject::new(
                    "library",
                    "Library",
                    "Library",
                    // BookxLibrary::static_type(),
                    glib::ParamFlags::READABLE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                "library" => obj.library().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    // Overrides Gio.Application for Bookx
    impl ApplicationImpl for BookxApplication {
        fn activate(&self, app: &Self::Type) {
            debug!("gio::Application -> activate()");
            let app = app.downcast_ref::<super::BookxApplication>().unwrap();

            // If the window already exists,
            // present it instead creating a new one again
            if let Some(weak_window) = self.window.get() {
                weak_window.upgrade().unwrap().present();
                info!("Application window presented");
                return;
            }

            // No window available -> create one
            let window = app.create_window();
            let _ = self.window.set(window.downgrade());
            info!("Created application window.");

            // Setup app level GActions
            app.setup_gactions();

            // Setup action channel
            let receiver = self.receiver.borrow_mut().take().unwrap();
            receiver.attach(
                None,
                clone!(@strong app => move |action| app.process_action(action)),
            );

            // Retreive Books data
            app.refresh_data();

            // Setup settings signal (we get notified when a key gets changed)
            self.settings.connect_changed(
                None,
                clone!(@strong self.sender as sender => move |_, key_str| {
                   let key: Key = Key::from_str(key_str).unwrap();
                   send!(sender, Action::SettingsKeyChanged(key));
                }),
            );

            // TODO: Needs to be called after settings.connect_changed for it to trigger.
            app.update_color_scheme();
        }

        fn startup(&self, app: &Self::Type) {
            debug!("GtkApplication<BookxApplication>::startup");
            self.parent_startup(app);
        }
    }

    // This is empty, but we still need to provide an
    // empty implementation for each type we subclass.
    impl GtkApplicationImpl for BookxApplication {}
    impl AdwApplicationImpl for BookxApplication {}
}

// Creates a wrapper struct that inherits the functions
// from objects listed as @extends or interfaces it @implements.
// This is what allows us to do e.g. application.quit() on
// BookxApplication without casting.
glib::wrapper! {
    pub struct BookxApplication(ObjectSubclass<imp::BookxApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

// This is where the member functions of BookxApplication go.
impl BookxApplication {
    pub fn run() {
        info!("Bookx ({})", config::APP_ID);
        info!("Version: {} ({})", config::VERSION, config::PROFILE);
        info!("Datadir: {}", config::DATADIR);

        // Create new GObject and downcast it into BookxApplication
        let app = glib::Object::new::<BookxApplication>(&[
            ("aplication-id", &Some(config::APP_ID)),
            ("flag", &gio::ApplicationFlags::empty()),
            ("resource-base-path", &Some("/com/anuragdhadse/Bookx")),
        ])
        .expect("Application initialization failed...");

        // Start running gtk::Application
        app.run();
    }

    fn main_window(&self) -> BookxWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    fn create_window(&self) -> BookxWindow {
        let imp = self.imp();
        let window = BookxWindow::new(imp.sender.clone(), self.clone());

        // Set initial view
        window.set_view(BookxView::Library);

        window.present();
        window
    }

    fn setup_gactions(&self) {
        // action! is a macro from gtk_macros
        // that creates a GSimpleAction with a callback.
        // clone! is a macro from glib-rs that allows
        // you to easily handle references in callbacks
        // without refcycles or leaks.
        //
        // When you don't want the callback to keep the
        // Object alive, pass as @weak. Otherwise, pass
        // as @strong. Most of the time you will want
        // to use @weak.

        action!(
            self,
            "about",
            clone!(@weak self as app => move |_, _| {
                app.show_about();
            })
        );

        action!(
            self,
            "help",
            clone!(@weak self as app => move |_, _| {
                app.show_help();
            })
        );

        // Sets up keyboard shortcuts
        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("app.quit", &["<Primary>Q"]);
        self.set_accels_for_action("app.new-window", &["<Primary>N"]);
        self.set_accels_for_action("win.open", &["<Primary>O"]);
        // self.set_accels_for_action("win.print", &["<Primary>P"]);
        self.set_accels_for_action("win.copy", &["<Primary>C"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Primary>question"]);
        self.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
        self.set_accels_for_action("window.close", &["<Primary>W"]);
    }

    // TODO: library(&self) -> Database::BookxLibrary
    // pub fn library(&self) -> BookxLibrary {}

    fn process_action(&self, action: Action) -> glib::Continue {
        let imp = self.imp();
        if self.active_window().is_none() {
            return glib::Continue(true);
        }

        let window = BookxWindow::default();

        match action {
            Action::SettingsKeyChanged(key) => self.apply_settings_changes(key),
        }
        glib::Continue(true)
    }

    fn apply_settings_changes(&self, key: Key) {
        debug!("Settings key changed: {:?}", &key);
        match key {
            Key::DarkMode => self.update_color_scheme(),
            _ => (),
        }
    }

    fn update_color_scheme(&self) {
        let manager = adw::StyleManager::default();
        if !manager.system_supports_color_schemes() {
            let color_scheme = if settings_manager::boolean(Key::DarkMode) {
                adw::ColorScheme::PreferDark
            } else {
                adw::ColorScheme::PreferLight
            };
            manager.set_color_scheme(color_scheme);
        }
    }

    // TODO -> refresh_data(): Retrieve books data
    pub fn refresh_data(&self) {}

    fn show_about(&self) {
        // Uncomment and delete the similar code when libadwaita 0.2 comes out of alpha release
        // let about = adw::AboutWindow::builder()
        //     .application_icon("Bookx")
        //     .application_icon(config::APP_ID)
        //     .license_type(gtk::License::Apache20)
        //     .website("https://bookx.anuragdhadse.com/")
        //     .issue_url("https://github.com/adhadse/Bookx/issues/")
        //     .version(config::VERSION)
        //     .translator_credits(String::from("translator-credits"))
        //     .copyright("© 2022 Anurag Dhadse")
        //     .developers(vec![
        //         String::from("Anurag Dhadse")
        //     ])
        //     .designers(vec![
        //         String::from("Anurag Dhadse")
        //     ])
        //     .build();

        // if let Some(window) = self.active_window() {
        //     about.set_transient_for(Some(&window));
        // }

        let about = gtk::AboutDialog::builder()
            .authors(vec![String::from("Anurag Dhadse")])
            .artists(vec![String::from("Anurag Dhadse")])
            .copyright("© 2022 Anurag Dhadse")
            .license_type(gtk::License::Apache20)
            .program_name("Bookx")
            .logo_icon_name(config::APP_ID)
            .version(config::VERSION)
            .build();

        if let Some(window) = self.active_window() {
            about.set_modal(true);
            about.set_transient_for(Some(&window));
        }

        about.show();
    }

    pub fn show_help(&self) {
        gtk::show_uri(self.active_window().as_ref(), "help:bookx", 0);
    }
}

impl Default for BookxApplication {
    fn default() -> Self {
        gio::Application::default()
            .expect("Could not get default GApplication")
            .downcast()
            .unwrap()
    }
}
