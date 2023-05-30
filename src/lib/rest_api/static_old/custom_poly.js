function isEventAtCoordinates(event, coordinates) {
    if (!event.lngLat) return false;
    return event.lngLat.lng === coordinates[0] && event.lngLat.lat === coordinates[1];
}

function createVertex(parentId, coordinates, path, selected) {
    return {
      type: 'feature',
      properties: {
        meta: 'vertex',
        parent: parentId,
        coord_path: path,
        active: (selected) ? 'true' : 'false'
      },
      geometry: {
        type: 'Point',
        coordinates
      }
    };
}

const doubleClickZoom = {
    enable: ctx => {
      setTimeout(() => {
        // First check we've got a map and some context.
        if (
          !ctx.map ||
          !ctx.map.doubleClickZoom ||
          !ctx._ctx ||
          !ctx._ctx.store ||
          !ctx._ctx.store.getInitialConfigValue
        )
          return;
  
        if (!ctx._ctx.store.getInitialConfigValue("doubleClickZoom")) return;
        ctx.map.doubleClickZoom.enable();
      }, 0);
    },
    disable(ctx) {
      console.log('clicked dd')
      setTimeout(() => {
        if (!ctx.map || !ctx.map.doubleClickZoom) return;
  
        ctx.map.doubleClickZoom.disable();
      }, 0);
    }
};

const CommonSelectors = {};

CommonSelectors.isOfMetaType = function (type) {
return function(e) {
    const featureTarget = e.featureTarget;
    if (!featureTarget) return false;
    if (!featureTarget.properties) return false;
    return featureTarget.properties.meta === type;
};
}

CommonSelectors.isShiftMousedown = function (e) {
    if (!e.originalEvent) return false;
    if (!e.originalEvent.shiftKey) return false;
    return e.originalEvent.button === 0;
}

CommonSelectors.isActiveFeature = function (e) {
    if (!e.featureTarget) return false;
    if (!e.featureTarget.properties) return false;
    return e.featureTarget.properties.active === 'true' &&
    e.featureTarget.properties.meta === 'feature';
}

CommonSelectors.isInactiveFeature = function (e) {
    if (!e.featureTarget) return false;
    if (!e.featureTarget.properties) return false;
    return e.featureTarget.properties.active === 'false' &&
    e.featureTarget.properties.meta === 'feature';
}

CommonSelectors.noTarget = function (e) {
    return e.featureTarget === undefined;
}

CommonSelectors.isFeature = function (e) {
    if (!e.featureTarget) return false;
    if (!e.featureTarget.properties) return false;
    return e.featureTarget.properties.meta === 'feature';
}

CommonSelectors.isVertex = function (e) {
    const featureTarget = e.featureTarget;
    if (!featureTarget) {
        return false;
    }
    if (!featureTarget.properties) {
        return false;
    }
    return featureTarget.properties.meta === 'vertex';
}

CommonSelectors.isShiftDown = function (e) {
    if (!e.originalEvent) return false;
    return e.originalEvent.shiftKey === true;
}

CommonSelectors.isEscapeKey = function (e) {
    return e.keyCode === 27;
}

CommonSelectors.isEnterKey = function (e) {
    return e.keyCode === 13;
}

CommonSelectors.isTrue = function () {
    return true;
}

const PolygonFourPointsOnly = MapboxDraw.modes.draw_polygon;
PolygonFourPointsOnly.maxVertices = 4;

PolygonFourPointsOnly.clickAnywhere = function(state, e) {
  if (state.currentVertexPosition > 0 && isEventAtCoordinates(e, state.polygon.coordinates[0][state.currentVertexPosition - 1])) {
    return this.changeMode('simple_select', { featureIds: [state.polygon.id] });
  }
  this.updateUIClasses({ mouse: 'add' });
  state.polygon.updateCoordinate(`0.${state.currentVertexPosition}`, e.lngLat.lng, e.lngLat.lat);
  state.currentVertexPosition++;
  state.polygon.updateCoordinate(`0.${state.currentVertexPosition}`, e.lngLat.lng, e.lngLat.lat);
  if (state.currentVertexPosition > this.maxVertices - 1) {
    this.updateUIClasses({
      mouse: "none"
    });
    return this.changeMode("simple_select", {
      featuresId: state.polygon.id
    });
  }
};

// PolygonFourPointsOnly.toDisplayFeatures = function(state, geojson, display) {
//   const isActivePolygon = geojson.properties.id === state.polygon.id;
//   geojson.properties.active = (isActivePolygon) ? 'true' : 'false';
//   if (!isActivePolygon) return display(geojson);

//   // Don't render a polygon until it has two positions
//   // (and a 3rd which is just the first repeated)
//   if (geojson.geometry.coordinates.length === 0) return;

//   const coordinateCount = geojson.geometry.coordinates[0].length;
//   // 2 coordinates after selecting a draw type
//   // 3 after creating the first point
//   if (coordinateCount < 3) {
//     return;
//   }
//   geojson.properties.meta = 'feature';
//   display(createVertex(state.polygon.id, geojson.geometry.coordinates[0][0], '0.0', false));
//   if (coordinateCount > 3) {
//     // Add a start position marker to the map, clicking on this will finish the feature
//     // This should only be shown when we're in a valid spot
//     const endPos = geojson.geometry.coordinates[0].length - 3;
//     display(createVertex(state.polygon.id, geojson.geometry.coordinates[0][endPos], `0.${endPos}`, false));
//   }
//   if (coordinateCount <= 4) {
//     // If we've only drawn two positions (plus the closer),
//     // make a LineString instead of a Polygon
//     const lineCoordinates = [
//       [geojson.geometry.coordinates[0][0][0], geojson.geometry.coordinates[0][0][1]], [geojson.geometry.coordinates[0][1][0], geojson.geometry.coordinates[0][1][1]]
//     ];
//     // create an initial vertex so that we can track the first point on mobile devices
//     display({
//       type: 'Feature',
//       properties: geojson.properties,
//       geometry: {
//         coordinates: lineCoordinates,
//         type: 'LineString'
//       }
//     });
//     if (coordinateCount === 3) {
//       return;
//     }
//   }
//   // render the Polygon
//   return display(geojson);
// };

// PolygonFourPointsOnly.onSetup = function() {
//   const polygon = this.newFeature({
//     type: 'Feature',
//     properties: {},
//     geometry: {
//       type: 'Polygon',
//       coordinates: [[]]
//     }
//   });

//   this.addFeature(polygon);
//   this.clearSelectedFeatures();
//   doubleClickZoom.disable(this);
//   this.updateUIClasses({ mouse: 'add' });
//   this.setActionableState({
//     trash: true
//   });

//   return {
//     polygon,
//     currentVertexPosition: 0
//   };
// };

// PolygonFourPointsOnly.onTrash = function(state) {
//   this.deleteFeature([state.polygon.id], { silent: true });
//   this.changeMode('simple_select');
// };

// PolygonFourPointsOnly.clickOnVertex = function(state) {
//   return this.changeMode('simple_select', { featureIds: [state.polygon.id] });
// };

// PolygonFourPointsOnly.onMouseMove = function(state, e) {
//   state.polygon.updateCoordinate(`0.${state.currentVertexPosition}`, e.lngLat.lng, e.lngLat.lat);
//   if (CommonSelectors.isVertex(e)) {
//     this.updateUIClasses({ mouse: 'pointer' });
//   }
// };

// PolygonFourPointsOnly.onTap = PolygonFourPointsOnly.onClick = function(state, e) {
//   if (CommonSelectors.isVertex(e)) return this.clickOnVertex(state, e);
//   return this.clickAnywhere(state, e);
// };

// PolygonFourPointsOnly.onKeyUp = function(state, e) {
//   if (CommonSelectors.isEscapeKey(e)) {
//     this.deleteFeature([state.polygon.id], { silent: true });
//     this.changeMode('simple_select');
//   } else if (CommonSelectors.isEnterKey(e)) {
//     this.changeMode('simple_select', { featureIds: [state.polygon.id] });
//   }
// };

// PolygonFourPointsOnly.onStop = function(state) {
//   this.updateUIClasses({ mouse: 'none' });
//   doubleClickZoom.enable(this);
//   this.activateUIButton();

//   // check to see if we've deleted this feature
//   if (this.getFeature(state.polygon.id) === undefined) return;

//   //remove last added coordinate
//   state.polygon.removeCoordinate(`0.${state.currentVertexPosition}`);
//   if (state.polygon.isValid()) {
//     this.map.fire('draw.create', {
//       features: [state.polygon.toGeoJSON()]
//     });
//   } else {
//     this.deleteFeature([state.polygon.id], { silent: true });
//     this.changeMode('simple_select', {}, { silent: true });
//   }
// };
