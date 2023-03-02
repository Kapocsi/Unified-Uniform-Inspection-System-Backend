#![warn(unused_imports, dead_code)]

pub mod data {
    use std::fs;

    use actix_web::HttpResponse;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Flight {
        Beddoe,
        Morgan,
        Spear,
        Bell,
        Hill,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Criteria {
        PassFail(CriteriaPassFail),
        Graded(CriteriaGraded),
        Comment(Option<String>),
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CriteriaPassFail {
        pub category_name: String,
        pub description: String,
        pub state: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CriteriaGraded {
        /// This is where the text in the rubric should be scored, their index is taken to be the
        /// score
        pub category_name: String,
        pub description: Vec<String>,
        pub state: Option<u8>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Inspection {
        pub name: String,
        pub criteria: Vec<Criteria>,
        pub date: Option<i64>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct User {
        // User Info
        pub username: Option<String>,
        pub uuid: String,
        pub inspections: Vec<Inspection>,
        pub flight: Option<Flight>,
        pub dev_user: bool,
    }

    impl User {
        pub fn new() -> User {
            let new_user = User {
                username: None,
                uuid: Uuid::new_v4().to_string(),
                inspections: Vec::new(),
                flight: None,
                // REMOVE THIS FLAG LATER
                dev_user: false,
                // REMOVE THIS FLAG LATER
            };
            new_user
        }
        pub fn push_to_data_base(&self) {
            fs::write(
                format!("database/users/{}.json", &self.uuid),
                serde_json::ser::to_string(&self).expect("Failed to serilize user"),
            )
            .expect("failed to write to disk");

            index_users().expect("Failed to index");
        }
        pub fn push_inspection(&mut self, inspec: Inspection) {
            let mut inspect = inspec;
            if inspect.date.is_none() {
                inspect.date = Some(chrono::Utc::now().timestamp())
            }
            self.inspections.push(inspect);
        }
        pub fn read_from_database(uuid: String) -> Result<User, HttpResponse> {
            let mut user: User = serde_json::de::from_str(
                fs::read_to_string(format!("./database/users/{}.json", uuid))
                    .map_err(|_| actix_web::HttpResponse::NotFound())?
                    .as_str(),
            )
            .map_err(|_| actix_web::HttpResponse::NotFound())?;

            let mut inspections = user.inspections;

            inspections.sort_by_key(|f| f.date.unwrap_or(0));
            inspections.reverse();

            user.inspections = inspections;

            Ok(user)
        }
    }

    pub fn load_inspection_list() -> Result<Vec<Inspection>, std::io::Error> {
        let inspection_lists: Vec<Inspection> =
            serde_json::from_str(fs::read_to_string("./database/inspections.json")?.as_str())
                .expect("Invalid Yaml in Inspection List");

        Ok(inspection_lists)
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct FlightIndexItem {
        user_uuid: String,
        flight: Option<Flight>,
        name: Option<String>,
        latest_inspection_date: Option<i64>,
    }

    impl From<&User> for FlightIndexItem {
        fn from(value: &User) -> Self {
            let value = value.clone();
            Self {
                user_uuid: value.uuid,
                name: value.username,
                flight: value.flight,
                latest_inspection_date: value
                    .inspections
                    .last()
                    .unwrap_or(&Inspection {
                        name: "PLACEHOLDER".into(),
                        criteria: vec![],
                        date: Some(169420),
                    })
                    .date,
            }
        }
    }

    impl From<User> for FlightIndexItem {
        fn from(value: User) -> Self {
            let last_inspection = value
                .inspections
                .last()
                .unwrap_or(&Inspection {
                    name: "BLANK".into(),
                    criteria: vec![],
                    date: Some(269420),
                })
                .date;
            Self {
                user_uuid: value.uuid,
                name: value.username,
                flight: value.flight,
                latest_inspection_date: last_inspection,
            }
        }
    }

    /// Reads all user and stores their uuid and flight
    pub fn index_users() -> Result<Vec<FlightIndexItem>, std::io::Error> {
        let files = fs::read_dir("./database/users/")?;
        let users: Vec<FlightIndexItem> = files
            .into_iter()
            .filter_map(|x| x.ok())
            .filter_map(|x| {
                serde_json::from_str::<User>(fs::read_to_string(x.path()).ok()?.as_str()).ok()
            })
            .map(|x| FlightIndexItem {
                user_uuid: x.uuid,
                flight: x.flight,
                name: x.username,
                latest_inspection_date: x
                    .inspections
                    .last()
                    .unwrap_or(&Inspection {
                        name: "PLACEHOLDER".into(),
                        criteria: vec![],
                        date: Some(369420),
                    })
                    .date,
            })
            .collect();

        fs::write(
            "./database/flight-index.json",
            serde_json::to_string(&users)?,
        )?;

        println!("{:#?}", users);

        Ok(users)
    }

    pub fn read_user_index() -> Result<Vec<FlightIndexItem>, std::io::Error> {
        Ok(serde_json::from_str::<Vec<FlightIndexItem>>(
            fs::read_to_string("./database/flight-index.json")?.as_str(),
        )?)
    }

    pub fn add_user_to_index(u: &User) -> Result<(), std::io::Error> {
        let mut users = read_user_index()?;
        users.push(u.into());

        fs::write(
            "./database/flight-index.json",
            serde_json::ser::to_string(&users)?,
        )
    }
}
