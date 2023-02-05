#![warn(unused_imports, dead_code)]

pub mod data {
    use std::fs;

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Criteria {
        PassFail(CriteriaPassFail),
        Graded(CriteriaGraded),
        Comment(Option<String>),
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CriteriaPassFail {
        pub category_name: String,
        pub description: String,
        pub state: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CriteriaGraded {
        /// This is where the text in the rubric should be scored, their index is taken to be the
        /// score
        pub category_name: String,
        pub descriptions: Vec<String>,
        pub state: Option<u8>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Inspection {
        pub name: String,
        pub criteria: Vec<Criteria>,
        pub date: Option<u64>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct User {
        // User Info
        pub username: Option<String>,
        pub uuid: String,
        pub inspections: Vec<Inspection>,
    }

    impl User {
        pub fn new() -> User {
            User {
                username: None,
                uuid: Uuid::new_v4().to_string(),
                inspections: Vec::new(),
            }
        }
        pub fn push_to_data_base(&self) {
            fs::write(
                format!("database/users/{}.json", &self.uuid),
                serde_json::ser::to_string(&self).expect("Failed to serilize user"),
            )
            .expect("failed to write to disk")
        }
        pub fn push_inspection(&mut self, inspec: Inspection) {
            self.inspections.push(inspec);
        }
        pub fn read_from_database(uuid: String) -> Result<User, actix_web::error::Error> {
            let user: User = serde_json::de::from_str(
                fs::read_to_string(format!("database/users/{}.json", uuid))
                    .map_err(|_| actix_web::error::ErrorNotFound("Could not find specified user"))?
                    .as_str(),
            )
            .expect("This should always be valid json");
            Ok(user)
        }
    }

    pub fn load_inspection_list() -> Result<Vec<Inspection>, std::io::Error> {
        let inspection_lists: Vec<Inspection> =
            serde_yaml::from_str(fs::read_to_string("./database/inspections.yaml")?.as_str())
                .expect("Invalid Yaml in Inspection List");

        Ok(inspection_lists)
    }
}
