#![warn(unused_imports, dead_code)]

pub mod data {
    use std::{fs};

    use actix_web::HttpResponse;
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
        pub description: Vec<String>,
        pub state: Option<u8>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Inspection {
        pub name: String,
        pub criteria: Vec<Criteria>,
        pub date: Option<i64>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct User {
        // User Info
        pub username: Option<String>,
        pub uuid: String,
        pub inspections: Vec<Inspection>,
        pub flight: Option<String>,
    }

    impl User {
        pub fn new() -> User {
            User {
                username: None,
                uuid: Uuid::new_v4().to_string(),
                inspections: Vec::new(),
                flight: None,
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
}
