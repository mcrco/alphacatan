#![allow(unsafe_op_in_unsafe_fn)]

use std::str::FromStr;

use pyo3::prelude::*;
use pyo3::sync::GILOnceCell;
use pyo3::types::{PyAny, PyDict, PyIterator, PyList, PyTuple};

use crate::board::MapType;
use crate::env::{Observation, PlayerObservation, RustEnv};
use crate::features::{build_board_tensor, collect_features};
use crate::game::{
    GameConfig, GameEvent, GameState, ResourceBundle,
    action::{ActionPayload, GameAction},
};
use crate::types::{ActionType, Color, Resource};
use std::collections::HashMap;

static ACTION_CLASS: GILOnceCell<Py<PyAny>> = GILOnceCell::new();
static ACTION_TYPE_CLASS: GILOnceCell<Py<PyAny>> = GILOnceCell::new();
static COLOR_CLASS: GILOnceCell<Py<PyAny>> = GILOnceCell::new();

#[pyclass(name = "CatanEnv")]
pub struct PyCatanEnv {
    inner: RustEnv,
}

#[pymethods]
impl PyCatanEnv {
    #[new]
    fn new(
        num_players: Option<usize>,
        map_type: Option<String>,
        vps_to_win: Option<u8>,
        seed: Option<u64>,
    ) -> PyResult<Self> {
        let mut config = GameConfig::default();
        if let Some(players) = num_players {
            config.num_players = players;
        }
        if let Some(vps) = vps_to_win {
            config.vps_to_win = vps;
        }
        if let Some(seed_val) = seed {
            config.seed = seed_val;
        }
        if let Some(map) = map_type {
            config.map_type = parse_map_type(&map)?;
        }
        Ok(Self {
            inner: RustEnv::new(config),
        })
    }

    fn reset(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        let obs = self.inner.reset();
        observation_to_py(py, &obs)
    }

    fn step<'py>(
        &mut self,
        py: Python<'py>,
        player_index: usize,
        action_type: &str,
        payload: Option<Bound<'py, PyAny>>,
    ) -> PyResult<(PyObject, Vec<f32>, bool, PyObject)> {
        let action = build_action(player_index, action_type, payload)?;
        let result = self
            .inner
            .step(action)
            .map_err(|err| PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string()))?;
        let obs_obj = observation_to_py(py, &result.observation)?;
        let events_obj = events_to_py(py, &result.events)?;
        Ok((obs_obj, result.rewards, result.done, events_obj))
    }

    fn pending_prompt(&self) -> String {
        self.inner.pending_prompt().to_string()
    }

    fn current_player(&self) -> usize {
        self.inner.current_player()
    }

    fn extract_features(
        &self,
        py: Python<'_>,
        player_index: usize,
    ) -> PyResult<(PyObject, PyObject, (usize, usize, usize))> {
        let (numeric, tensor) = self.inner.extract_features(player_index).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid player index")
        })?;
        let numeric_list = PyList::new_bound(py, numeric.values.iter().copied());
        let tensor_list = PyList::new_bound(py, tensor.data.iter().copied());
        Ok((
            numeric_list.into_py(py),
            tensor_list.into_py(py),
            (tensor.width, tensor.height, tensor.channels),
        ))
    }
}

fn parse_map_type(value: &str) -> PyResult<MapType> {
    MapType::from_str(value)
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid map type"))
}

#[pyclass(name = "PyRustGame")]
pub struct PyRustGame {
    state: GameState,
}

#[pymethods]
impl PyRustGame {
    #[new]
    fn new(
        num_players: Option<usize>,
        map_type: Option<String>,
        vps_to_win: Option<u8>,
        seed: Option<u64>,
    ) -> PyResult<Self> {
        let mut config = GameConfig::default();
        if let Some(players) = num_players {
            config.num_players = players;
        }
        if let Some(vps) = vps_to_win {
            config.vps_to_win = vps;
        }
        if let Some(seed_val) = seed {
            config.seed = seed_val;
        }
        if let Some(map) = map_type {
            config.map_type = parse_map_type(&map)?;
        }
        Ok(Self {
            state: GameState::new(config),
        })
    }

    fn copy(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }

    fn current_player_index(&self) -> usize {
        self.state.current_player
    }

    fn current_color(&self, py: Python<'_>) -> PyResult<PyObject> {
        let color = self.state.players[self.state.current_player].color;
        color_to_py(py, color)
    }

    fn legal_actions(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.state
            .legal_actions()
            .iter()
            .map(|action| game_action_to_py(py, &self.state, action))
            .collect()
    }

    fn apply_action(&mut self, py: Python<'_>, action: &PyAny) -> PyResult<PyObject> {
        let game_action = py_action_to_game_action(py, &self.state, action)?;
        self.state
            .step(game_action)
            .map_err(|err| PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string()))?;
        let last = self
            .state
            .action_log()
            .last()
            .expect("action log updated after step");
        game_action_to_py(py, &self.state, last)
    }

    fn winner(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let winner = match &self.state.phase {
            crate::game::state::GamePhase::Completed { winner } => winner
                .and_then(|idx| self.state.players.get(idx))
                .map(|player| player.color),
            _ => None,
        };
        match winner {
            Some(color) => color_to_py(py, color).map(Some),
            None => Ok(None),
        }
    }

    fn pending_prompt(&self) -> String {
        self.state.pending_prompt.to_string()
    }

    fn extract_features(
        &self,
        py: Python<'_>,
        player_index: usize,
    ) -> PyResult<(PyObject, PyObject, (usize, usize, usize))> {
        if player_index >= self.state.players.len() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "invalid player index",
            ));
        }
        let numeric = collect_features(&self.state, player_index);
        let tensor = build_board_tensor(&self.state, player_index);
        let numeric_values = numeric.numeric_values();
        let numeric_list = PyList::new_bound(py, numeric_values.iter().copied());
        let tensor_list = PyList::new_bound(py, tensor.data.iter().copied());
        Ok((
            numeric_list.into_py(py),
            tensor_list.into_py(py),
            (tensor.width, tensor.height, tensor.channels),
        ))
    }

    fn actions(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.state
            .action_log()
            .iter()
            .map(|action| game_action_to_py(py, &self.state, action))
            .collect()
    }

    fn node_signatures(&self) -> Vec<(u16, Vec<((i16, i16, i16), u8)>)> {
        use crate::types::NodeRef;

        fn node_ref_index(node_ref: NodeRef) -> u8 {
            match node_ref {
                NodeRef::North => 0,
                NodeRef::NorthEast => 1,
                NodeRef::SouthEast => 2,
                NodeRef::South => 3,
                NodeRef::SouthWest => 4,
                NodeRef::NorthWest => 5,
            }
        }

        let mut signatures: HashMap<u16, Vec<((i16, i16, i16), u8)>> = HashMap::new();
        for (coord, tile) in &self.state.map.land_tiles {
            let coord_tuple = (coord.x as i16, coord.y as i16, coord.z as i16);
            for (node_ref, node_id) in &tile.nodes {
                let entry = (coord_tuple, node_ref_index(*node_ref));
                signatures.entry(*node_id).or_default().push(entry);
            }
        }

        let mut result = Vec::new();
        for (node_id, mut signature) in signatures {
            signature.sort_unstable();
            result.push((node_id, signature));
        }
        result
    }
}

fn build_action(
    player_index: usize,
    action_str: &str,
    payload: Option<Bound<'_, PyAny>>,
) -> PyResult<GameAction> {
    let action_type = ActionType::from_str(action_str)
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid action type"))?;
    let payload = match action_type {
        ActionType::BuildSettlement | ActionType::BuildCity => {
            ActionPayload::Node(extract_node_id(payload)?)
        }
        ActionType::BuildRoad => ActionPayload::Edge(extract_edge(payload)?),
        ActionType::Roll => match payload {
            Some(obj) => {
                let tuple = obj.downcast::<PyTuple>().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("expected dice tuple")
                })?;
                if tuple.len() != 2 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "dice tuple must have two elements",
                    ));
                }
                let d1: u8 = tuple.get_item(0)?.extract().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid die value")
                })?;
                let d2: u8 = tuple.get_item(1)?.extract().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid die value")
                })?;
                ActionPayload::Dice(d1, d2)
            }
            None => ActionPayload::None,
        },
        _ => ActionPayload::None,
    };
    Ok(GameAction::new(player_index, action_type).with_payload(payload))
}

fn extract_node_id(payload: Option<Bound<'_, PyAny>>) -> PyResult<u16> {
    payload
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing node id"))?
        .extract::<u16>()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))
}

fn extract_edge(payload: Option<Bound<'_, PyAny>>) -> PyResult<(u16, u16)> {
    let obj = payload
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing edge payload"))?;
    let tuple = obj
        .downcast::<PyTuple>()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("edge must be tuple"))?;
    if tuple.len() != 2 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "edge payload must have two node ids",
        ));
    }
    let a: u16 = tuple
        .get_item(0)?
        .extract()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))?;
    let b: u16 = tuple
        .get_item(1)?
        .extract()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))?;
    Ok((a, b))
}

fn observation_to_py(py: Python<'_>, obs: &Observation) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("current_player", obs.current_player)?;
    dict.set_item("pending_prompt", obs.pending_prompt.to_string())?;
    dict.set_item("turn", obs.turn)?;
    if let Some(roll) = obs.last_roll {
        dict.set_item("last_roll", (roll.0, roll.1))?;
    } else {
        dict.set_item("last_roll", py.None())?;
    }

    let players = PyList::empty_bound(py);
    for player in &obs.players {
        players.append(player_to_dict(py, player)?)?;
    }
    dict.set_item("players", players)?;
    Ok(dict.into_py(py))
}

fn player_to_dict(py: Python<'_>, player: &PlayerObservation) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("color", player.color.to_string())?;
    dict.set_item("resources", player.resources.to_vec())?;
    dict.set_item("dev_cards", player.dev_cards)?;
    dict.set_item("fresh_dev_cards", player.fresh_dev_cards)?;
    dict.set_item("settlements", player.settlements)?;
    dict.set_item("cities", player.cities)?;
    dict.set_item("roads", player.roads)?;
    dict.set_item("victory_points", player.victory_points)?;
    Ok(dict.into_py(py))
}

fn events_to_py(py: Python<'_>, events: &[GameEvent]) -> PyResult<PyObject> {
    let list = PyList::empty_bound(py);
    for event in events {
        list.append(event_to_dict(py, event)?)?;
    }
    Ok(list.into_py(py))
}

fn event_to_dict(py: Python<'_>, event: &GameEvent) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    match event {
        GameEvent::DiceRolled { player, dice, sum } => {
            dict.set_item("event", "dice_rolled")?;
            dict.set_item("player", *player)?;
            dict.set_item("dice", (dice.0, dice.1))?;
            dict.set_item("sum", *sum)?;
        }
        GameEvent::ResourcesDistributed { player, bundle } => {
            dict.set_item("event", "resources")?;
            dict.set_item("player", *player)?;
            dict.set_item("resources", bundle.counts().to_vec())?;
        }
        GameEvent::BuiltRoad { player, edge } => {
            dict.set_item("event", "built_road")?;
            dict.set_item("player", *player)?;
            dict.set_item("edge", (edge.0, edge.1))?;
        }
        GameEvent::BuiltSettlement { player, node } => {
            dict.set_item("event", "built_settlement")?;
            dict.set_item("player", *player)?;
            dict.set_item("node", *node)?;
        }
        GameEvent::BuiltCity { player, node } => {
            dict.set_item("event", "built_city")?;
            dict.set_item("player", *player)?;
            dict.set_item("node", *node)?;
        }
        GameEvent::TurnAdvanced { next_player } => {
            dict.set_item("event", "turn_advanced")?;
            dict.set_item("next_player", *next_player)?;
        }
        GameEvent::GameWon { winner } => {
            dict.set_item("event", "game_won")?;
            dict.set_item("winner", *winner)?;
        }
    }
    Ok(dict.into_py(py))
}

fn action_class<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let reference = ACTION_CLASS.get_or_try_init(py, || {
        let module = py.import("catanatron.models.enums")?;
        let action = module.getattr("Action")?;
        Ok::<Py<PyAny>, PyErr>(action.into())
    })?;
    Ok(reference.bind(py).clone())
}

fn action_type_class<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let reference = ACTION_TYPE_CLASS.get_or_try_init(py, || {
        let module = py.import("catanatron.models.enums")?;
        let action_type = module.getattr("ActionType")?;
        Ok::<Py<PyAny>, PyErr>(action_type.into())
    })?;
    Ok(reference.bind(py).clone())
}

fn color_class<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let reference = COLOR_CLASS.get_or_try_init(py, || {
        let module = py.import("catanatron.models.player")?;
        let color = module.getattr("Color")?;
        Ok::<Py<PyAny>, PyErr>(color.into())
    })?;
    Ok(reference.bind(py).clone())
}

fn color_to_py(py: Python<'_>, color: Color) -> PyResult<PyObject> {
    let class = color_class(py)?;
    let attr = class.getattr(color.to_string().as_str())?;
    Ok(attr.into_py(py))
}

fn py_to_color(obj: &PyAny) -> PyResult<Color> {
    let value: String = obj
        .getattr("value")
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing color value"))?
        .extract()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid color value"))?;
    Color::from_str(&value)
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid color"))
}

fn action_type_to_py(py: Python<'_>, action_type: ActionType) -> PyResult<PyObject> {
    let class = action_type_class(py)?;
    let attr = class.getattr(action_type.to_string().as_str())?;
    Ok(attr.into_py(py))
}

fn py_to_action_type(obj: &PyAny) -> PyResult<ActionType> {
    let value: String = obj
        .getattr("value")
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing action type value"))?
        .extract()
        .map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid action type value")
        })?;
    ActionType::from_str(&value)
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid action type"))
}

fn resource_to_py(py: Python<'_>, resource: Resource) -> PyObject {
    resource.to_string().into_py(py)
}

fn py_to_resource(obj: &PyAny) -> PyResult<Resource> {
    let value: String = obj
        .extract()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid resource value"))?;
    Resource::from_str(&value)
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid resource"))
}

fn bundle_from_sequence(seq: &PyAny) -> PyResult<ResourceBundle> {
    let mut bundle = ResourceBundle::zero();
    let iterator = PyIterator::from_object(seq)?;
    for item in iterator {
        let obj = item?;
        let resource = py_to_resource(obj)?;
        bundle.add(resource, 1);
    }
    Ok(bundle)
}

fn bundle_to_sequence(py: Python<'_>, bundle: &ResourceBundle) -> PyObject {
    let mut items = Vec::new();
    for (resource, count) in bundle.iter() {
        for _ in 0..count {
            items.push(resource_to_py(py, resource));
        }
    }
    PyList::new_bound(py, items).into_py(py)
}

fn freq_tuple_to_bundle(tuple: &PyTuple, start: usize) -> PyResult<ResourceBundle> {
    let resource_count = Resource::ALL.len();
    if tuple.len() < start + resource_count {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "trade payload too short",
        ));
    }
    let mut bundle = ResourceBundle::zero();
    for (idx, resource) in Resource::ALL.iter().enumerate() {
        let value: i32 = tuple
            .get_item(start + idx)?
            .extract()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid freq value"))?;
        if value < 0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "freq values must be non-negative",
            ));
        }
        if value > 0 {
            bundle.add(*resource, value as u8);
        }
    }
    Ok(bundle)
}

fn bundle_to_freq_counts(bundle: &ResourceBundle) -> [i32; Resource::ALL.len()] {
    let mut counts = [0i32; Resource::ALL.len()];
    for (idx, resource) in Resource::ALL.iter().enumerate() {
        counts[idx] = bundle.get(*resource) as i32;
    }
    counts
}

fn maritime_tuple_to_payload(tuple: &PyTuple) -> PyResult<(ResourceBundle, Resource)> {
    if tuple.len() != 5 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "maritime trade tuple must have five entries",
        ));
    }
    let mut give = ResourceBundle::zero();
    for idx in 0..4 {
        let item = tuple.get_item(idx)?;
        if item.is_none() {
            continue;
        }
        let resource = py_to_resource(item)?;
        give.add(resource, 1);
    }
    let receive = py_to_resource(tuple.get_item(4)?)?;
    Ok((give, receive))
}

fn maritime_payload_to_py(
    py: Python<'_>,
    give: &ResourceBundle,
    receive: Resource,
) -> PyResult<PyObject> {
    let mut entries: Vec<PyObject> = Vec::with_capacity(5);
    for (resource, amount) in give.iter() {
        for _ in 0..amount {
            entries.push(resource_to_py(py, resource));
        }
    }
    while entries.len() < 4 {
        entries.push(py.None());
    }
    entries.push(resource_to_py(py, receive));
    Ok(PyTuple::new_bound(py, entries).into_py(py))
}

fn trade_payload_to_py(
    py: Python<'_>,
    state: &GameState,
    give: &ResourceBundle,
    receive: &ResourceBundle,
    partner: Option<usize>,
) -> PyResult<PyObject> {
    let mut values: Vec<PyObject> =
        Vec::with_capacity(Resource::ALL.len() * 2 + partner.map_or(0, |_| 1));
    for count in bundle_to_freq_counts(give) {
        values.push(count.into_py(py));
    }
    for count in bundle_to_freq_counts(receive) {
        values.push(count.into_py(py));
    }
    if let Some(idx) = partner {
        let color_obj = color_to_py(py, state.players[idx].color)?;
        values.push(color_obj);
    }
    Ok(PyTuple::new_bound(py, values).into_py(py))
}

fn color_to_player_index(state: &GameState, color: Color) -> Option<usize> {
    state
        .players
        .iter()
        .position(|player| player.color == color)
}

fn py_action_to_game_action(
    py: Python<'_>,
    state: &GameState,
    action: &PyAny,
) -> PyResult<GameAction> {
    let color_obj = action
        .getattr("color")
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("action missing color"))?;
    let color = py_to_color(color_obj)?;
    let player_index = color_to_player_index(state, color).ok_or_else(|| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>("color not in current game")
    })?;
    let action_type_obj = action.getattr("action_type").map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>("action missing action_type")
    })?;
    let action_type = py_to_action_type(action_type_obj)?;
    let payload_obj = action
        .getattr("value")
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("action missing value"))?;
    let payload = py_payload_to_action_payload(py, state, player_index, action_type, payload_obj)?;
    Ok(GameAction {
        player_index,
        action_type,
        payload,
    })
}

fn py_payload_to_action_payload(
    _py: Python<'_>,
    state: &GameState,
    _player_index: usize,
    action_type: ActionType,
    payload: &PyAny,
) -> PyResult<ActionPayload> {
    match action_type {
        ActionType::BuildSettlement | ActionType::BuildCity => {
            if payload.is_none() {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "missing node id",
                ));
            }
            let node: u16 = payload
                .extract()
                .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))?;
            Ok(ActionPayload::Node(node))
        }
        ActionType::BuildRoad => {
            let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("edge must be tuple")
            })?;
            if tuple.len() != 2 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "edge must have two nodes",
                ));
            }
            let a: u16 = tuple
                .get_item(0)?
                .extract()
                .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))?;
            let b: u16 = tuple
                .get_item(1)?
                .extract()
                .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid node id"))?;
            let edge = if a <= b { (a, b) } else { (b, a) };
            Ok(ActionPayload::Edge(edge))
        }
        ActionType::Roll => {
            if payload.is_none() {
                Ok(ActionPayload::None)
            } else {
                let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("dice must be tuple")
                })?;
                if tuple.len() != 2 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "dice must have two values",
                    ));
                }
                let d1: u8 = tuple.get_item(0)?.extract().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid die value")
                })?;
                let d2: u8 = tuple.get_item(1)?.extract().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid die value")
                })?;
                Ok(ActionPayload::Dice(d1, d2))
            }
        }
        ActionType::MoveRobber => {
            let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("robber payload must be tuple")
            })?;
            if tuple.len() != 3 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "robber payload must have three elements",
                ));
            }
            let tile_id: u16 = tuple
                .get_item(0)?
                .extract()
                .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid tile id"))?;
            let victim = if tuple.get_item(1)?.is_none() {
                None
            } else {
                let color_obj = tuple.get_item(1)?;
                let color = py_to_color(color_obj)?;
                Some(color_to_player_index(state, color).ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("victim color not in game")
                })?)
            };
            let stolen = if tuple.get_item(2)?.is_none() {
                None
            } else {
                Some(py_to_resource(tuple.get_item(2)?)?)
            };
            Ok(ActionPayload::Robber {
                tile_id,
                victim,
                resource: stolen,
            })
        }
        ActionType::Discard => {
            if payload.is_none() {
                Ok(ActionPayload::None)
            } else {
                let bundle = bundle_from_sequence(payload)?;
                Ok(ActionPayload::Resources(bundle))
            }
        }
        ActionType::MaritimeTrade => {
            let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "maritime trade payload must be tuple",
                )
            })?;
            let (give, receive) = maritime_tuple_to_payload(tuple)?;
            Ok(ActionPayload::MaritimeTrade { give, receive })
        }
        ActionType::OfferTrade => {
            let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("trade payload must be tuple")
            })?;
            let give = freq_tuple_to_bundle(tuple, 0)?;
            let receive = freq_tuple_to_bundle(tuple, Resource::ALL.len())?;
            Ok(ActionPayload::Trade {
                give,
                receive,
                partner: None,
            })
        }
        ActionType::ConfirmTrade => {
            let tuple = payload.downcast::<PyTuple>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("trade payload must be tuple")
            })?;
            if tuple.len() != Resource::ALL.len() * 2 + 1 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "confirm trade payload must include freq decks and color",
                ));
            }
            let give = freq_tuple_to_bundle(tuple, 0)?;
            let receive = freq_tuple_to_bundle(tuple, Resource::ALL.len())?;
            let color_obj = tuple.get_item(Resource::ALL.len() * 2)?;
            let color = py_to_color(color_obj)?;
            let partner = color_to_player_index(state, color).ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("unknown partner")
            })?;
            Ok(ActionPayload::Trade {
                give,
                receive,
                partner: Some(partner),
            })
        }
        ActionType::AcceptTrade | ActionType::RejectTrade => Ok(ActionPayload::None),
        ActionType::PlayYearOfPlenty => {
            if payload.is_none() {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "year of plenty requires resources",
                ));
            }
            let bundle = bundle_from_sequence(payload)?;
            let total = bundle.total();
            if total == 0 || total > 2 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "year of plenty must select one or two resources",
                ));
            }
            Ok(ActionPayload::Resources(bundle))
        }
        ActionType::PlayMonopoly => {
            if payload.is_none() {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "monopoly requires resource",
                ));
            }
            let resource = py_to_resource(payload)?;
            Ok(ActionPayload::Resource(resource))
        }
        ActionType::PlayKnightCard
        | ActionType::PlayRoadBuilding
        | ActionType::BuyDevelopmentCard
        | ActionType::EndTurn => Ok(ActionPayload::None),
        _ => Ok(ActionPayload::None),
    }
}

fn game_action_to_py(py: Python<'_>, state: &GameState, action: &GameAction) -> PyResult<PyObject> {
    let action_cls = action_class(py)?;
    let color = state.players[action.player_index].color;
    let color_obj = color_to_py(py, color)?;
    let action_type_obj = action_type_to_py(py, action.action_type)?;
    let payload = action_payload_to_py(py, state, action)?;
    Ok(action_cls
        .call1((color_obj, action_type_obj, payload))?
        .into_py(py))
}

fn action_payload_to_py(
    py: Python<'_>,
    state: &GameState,
    action: &GameAction,
) -> PyResult<PyObject> {
    match (&action.action_type, &action.payload) {
        (ActionType::BuildSettlement | ActionType::BuildCity, ActionPayload::Node(node)) => {
            Ok(node.into_py(py))
        }
        (ActionType::BuildRoad, ActionPayload::Edge(edge)) => Ok((edge.0, edge.1).into_py(py)),
        (ActionType::Roll, ActionPayload::Dice(a, b)) => Ok((*a, *b).into_py(py)),
        (ActionType::Roll, _) => Ok(py.None()),
        (
            ActionType::MoveRobber,
            ActionPayload::Robber {
                tile_id,
                victim,
                resource,
            },
        ) => {
            let victim_obj = victim
                .and_then(|idx| state.players.get(idx))
                .map(|player| color_to_py(py, player.color))
                .transpose()?
                .unwrap_or_else(|| py.None());
            let resource_obj = match resource {
                Some(res) => resource_to_py(py, *res),
                None => py.None(),
            };
            Ok(PyTuple::new_bound(py, [tile_id.into_py(py), victim_obj, resource_obj]).into_py(py))
        }
        (ActionType::Discard, ActionPayload::Resources(bundle)) => {
            Ok(bundle_to_sequence(py, bundle))
        }
        (ActionType::PlayYearOfPlenty, ActionPayload::Resources(bundle)) => {
            Ok(bundle_to_sequence(py, bundle))
        }
        (ActionType::PlayMonopoly, ActionPayload::Resource(resource)) => {
            Ok(resource_to_py(py, *resource))
        }
        (ActionType::MaritimeTrade, ActionPayload::MaritimeTrade { give, receive }) => {
            maritime_payload_to_py(py, give, *receive)
        }
        (
            ActionType::OfferTrade | ActionType::ConfirmTrade,
            ActionPayload::Trade {
                give,
                receive,
                partner,
            },
        ) => trade_payload_to_py(py, state, give, receive, *partner),
        (_, ActionPayload::None) => Ok(py.None()),
        _ => Ok(py.None()),
    }
}

#[allow(deprecated)]
#[pymodule]
pub fn catanatron_rs(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyCatanEnv>()?;
    m.add_class::<PyRustGame>()?;
    Ok(())
}

pub fn create_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
    let module = PyModule::new_bound(py, "catanatron_rs")?;
    module.add_class::<PyCatanEnv>()?;
    module.add_class::<PyRustGame>()?;
    Ok(module)
}
