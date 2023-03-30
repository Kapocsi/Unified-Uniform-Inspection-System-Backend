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

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum Criteria {
        PassFail(CriteriaPassFail),
        Graded(CriteriaGraded),
        Comment(Option<String>),
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct CriteriaPassFail {
        pub category_name: String,
        pub description: String,
        pub state: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct CriteriaGraded {
        /// This is where the text in the rubric should be scored, their index is taken to be the
        /// score
        pub category_name: String,
        pub description: Vec<String>,
        pub state: Option<u8>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Inspection {
        pub name: String,
        pub criteria: Vec<Criteria>,
        pub date: Option<i64>,
        pub out_of: Option<u16>,
        pub score: Option<u16>,
    }

    impl Default for Inspection {
        fn default() -> Self {
            Self {
                name: "".into(),
                criteria: vec![],
                date: None,
                out_of: None,
                score: None,
            }
        }
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

    impl Inspection {
        pub fn compute_score(self: &mut Self) {
            let true_false_map = |x: Option<bool>| match x {
                Some(true) => 1,
                Some(false) | None => 0,
            };

            let out_of_map = |x: &Criteria| -> u16 {
                match x {
                    Criteria::Graded(t) => t.description.len() as u16,
                    Criteria::PassFail(_) => 1,
                    Criteria::Comment(_) => 0,
                }
            };
            let score_map = |x: &Criteria| -> u16 {
                match x {
                    Criteria::Graded(t) => t.state.unwrap_or(0).into(),
                    Criteria::PassFail(t) => true_false_map(t.state),
                    Criteria::Comment(_) => 0,
                }
            };

            self.score = Some(self.criteria.iter().map(score_map).sum());
            self.out_of = Some(self.criteria.iter().map(out_of_map).sum());
        }

        pub fn get_score(self: &Self) -> InspectionScore {
            let true_false_map = |x: Option<bool>| match x {
                Some(true) => 1,
                Some(false) | None => 0,
            };

            let out_of_map = |x: &Criteria| -> u16 {
                match x {
                    Criteria::Graded(t) => t.description.len() as u16,
                    Criteria::PassFail(_) => 1,
                    Criteria::Comment(_) => 0,
                }
            };
            let score_map = |x: &Criteria| -> u16 {
                match x {
                    Criteria::Graded(t) => t.state.unwrap_or(0).into(),
                    Criteria::PassFail(t) => true_false_map(t.state),
                    Criteria::Comment(_) => 0,
                }
            };

            let score = self.criteria.iter().map(score_map).sum();
            let out_of = self.criteria.iter().map(out_of_map).sum();

            InspectionScore { score, out_of }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InspectionScore {
        score: u16,
        out_of: u16,
    }

    impl User {
        fn get_latest_inspection_date(&self) -> Option<i64> {
            self.inspections.last()?.date
        }

        fn get_latest_inspection_score(&self) -> Option<InspectionScore> {
            Some(self.inspections.last()?.get_score())
        }

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

            inspect.compute_score();

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
            inspections.iter_mut().for_each(|f| f.compute_score());

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
        latest_inspection_score: Option<InspectionScore>,
    }

    impl From<&User> for FlightIndexItem {
        fn from(value: &User) -> Self {
            let value = value.clone();
            let latest_inspection_date = value.get_latest_inspection_date();
            let latest_inspection_score = value.get_latest_inspection_score();
            Self {
                user_uuid: value.uuid,
                name: value.username,
                flight: value.flight,
                latest_inspection_date,
                latest_inspection_score,
            }
        }
    }

    impl From<User> for FlightIndexItem {
        fn from(value: User) -> Self {
            let last_inspection = value.get_latest_inspection_date();
            let latest_inspection_score = value.get_latest_inspection_score();
            Self {
                user_uuid: value.uuid,
                name: value.username,
                flight: value.flight,
                latest_inspection_date: last_inspection,
                latest_inspection_score,
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
            .map(|x| {
                let latest_inspection_date = x.get_latest_inspection_date();
                let latest_inspection_score = x.get_latest_inspection_score();
                FlightIndexItem {
                    user_uuid: x.uuid,
                    flight: x.flight,
                    name: x.username,
                    latest_inspection_date,
                    latest_inspection_score,
                }
            })
            .collect();

        fs::write(
            "./database/flight-index.json",
            serde_json::to_string(&users)?,
        )?;

        Ok(users)
    }

    pub fn read_user_index() -> Result<Vec<FlightIndexItem>, std::io::Error> {
        Ok(serde_json::from_str::<Vec<FlightIndexItem>>(
            fs::read_to_string("./database/flight-index.json")?.as_str(),
        )?)
    }

    // pub fn add_user_to_index(u: &User) -> Result<(), std::io::Error> {
    //     let mut users = read_user_index()?;
    //     users.push(u.into());
    //
    //     fs::write(
    //         "./database/flight-index.json",
    //         serde_json::ser::to_string(&users)?,
    //     )
    // }
}
